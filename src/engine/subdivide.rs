// Catmull-Clark subdivision surface.
// See docs/research/procedural-modeling.md for algorithm decisions and references.
//
// Each application of catmull_clark() replaces every n-gon with n quads.
// After one application, the mesh is all-quad regardless of input polygon type.
//
// Vertex count formula for closed all-quad mesh: V_new = V + E + F
// Cube verification:
//   Level 0:  8 verts,  6 faces,  12 edges
//   Level 1: 26 verts, 24 faces,  48 edges
//   Level 2: 98 verts, 96 faces, 192 edges

use std::collections::HashMap;
use glam::Vec3;
use super::mesh::PolyMesh;

// ============================================================================
// EDGE UTILITIES
// ============================================================================

/// Canonical key for an undirected edge: always (min, max).
/// This ensures (a,b) and (b,a) map to the same entry.
fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Per-edge data computed during Catmull-Clark.
struct EdgeEntry {
    /// Indices of the 1 or 2 faces adjacent to this edge.
    /// Interior edges have 2; boundary edges have 1.
    adjacent_faces: Vec<usize>,
    /// Index of this edge's edge-point in the output mesh. Set during Phase 2.
    new_idx: usize,
}

type EdgeMap = HashMap<(usize, usize), EdgeEntry>;

// ============================================================================
// PUBLIC API
// ============================================================================

/// Apply one level of Catmull-Clark subdivision to a PolyMesh.
/// Input can be any polygon mesh; output is always all-quad.
/// Winding (CCW) is preserved through all phases.
pub fn catmull_clark(mesh: &PolyMesh) -> PolyMesh {
    let n_verts = mesh.vertex_count();

    // ---- Phase 0: Build adjacency ----------------------------------------
    // vertex_faces[v] = list of face indices adjacent to vertex v
    // vertex_edges[v] = list of canonical edge keys incident to vertex v
    // edge_map: edge → adjacent faces + placeholder for new index

    let mut vertex_faces: Vec<Vec<usize>> = vec![vec![]; n_verts];
    let mut vertex_edges: Vec<Vec<(usize, usize)>> = vec![vec![]; n_verts];
    let mut edge_map: EdgeMap = HashMap::new();

    for (fi, face) in mesh.faces.iter().enumerate() {
        let n = face.len();
        for (i, &vi) in face.iter().enumerate() {
            // Track which faces touch each vertex
            vertex_faces[vi].push(fi);

            // Build edge_map for each edge in this face
            let vj = face[(i + 1) % n];
            let key = edge_key(vi, vj);
            let entry = edge_map.entry(key).or_insert_with(|| EdgeEntry {
                adjacent_faces: Vec::new(),
                new_idx: 0,
            });
            if !entry.adjacent_faces.contains(&fi) {
                entry.adjacent_faces.push(fi);
            }

            // Track which edges touch each vertex (avoid duplicates)
            if !vertex_edges[vi].contains(&key) {
                vertex_edges[vi].push(key);
            }
            if !vertex_edges[vj].contains(&key) {
                vertex_edges[vj].push(key);
            }
        }
    }

    // ---- Phase 1: Compute face centroids (stored as Vec3, not yet in output mesh) ---
    let face_centroids: Vec<Vec3> = mesh.faces.iter().map(|face| {
        let sum: Vec3 = face.iter().map(|&vi| mesh.positions[vi]).sum();
        sum / face.len() as f32
    }).collect();

    // ---- Output mesh -------------------------------------------------------
    let mut out = PolyMesh::new();

    // ---- Phase 2: Edge points → add to output mesh, record new indices -----
    // ep = (v0 + v1 + face_centroid_0 + face_centroid_1) / 4  (interior edge)
    // ep = (v0 + v1) / 2                                       (boundary edge)
    for ((a, b), entry) in edge_map.iter_mut() {
        let pa = mesh.positions[*a];
        let pb = mesh.positions[*b];
        let ep = if entry.adjacent_faces.len() == 2 {
            (pa + pb + face_centroids[entry.adjacent_faces[0]] + face_centroids[entry.adjacent_faces[1]]) / 4.0
        } else {
            // Boundary edge (should not occur for closed meshes like the cube)
            (pa + pb) / 2.0
        };
        entry.new_idx = out.add_vertex(ep);
    }

    // ---- Phase 3: Updated original vertex positions → add to output mesh ---
    // Catmull-Clark vertex update formula (interior vertex, valence n):
    //   F = average of adjacent face centroids
    //   R = average of adjacent edge midpoints
    //   new_V = (F + 2R + (n-3) * V) / n
    let mut new_v_idx: Vec<usize> = vec![0; n_verts];
    for v in 0..n_verts {
        let adj_faces = &vertex_faces[v];
        let n = adj_faces.len() as f32;

        // F: average adjacent face centroids
        let f: Vec3 = adj_faces.iter().map(|&fi| face_centroids[fi]).sum::<Vec3>() / n;

        // R: average adjacent edge midpoints
        let r: Vec3 = vertex_edges[v].iter()
            .map(|&(a, b)| (mesh.positions[a] + mesh.positions[b]) / 2.0)
            .sum::<Vec3>() / n;

        let new_pos = (f + 2.0 * r + (n - 3.0) * mesh.positions[v]) / n;
        new_v_idx[v] = out.add_vertex(new_pos);
    }

    // ---- Phase 1b: Face points → add to output mesh ------------------------
    let face_point_idx: Vec<usize> = face_centroids.iter()
        .map(|&c| out.add_vertex(c))
        .collect();

    // ---- Phase 4: Reconstruct faces ----------------------------------------
    // Each old n-gon face [v0, v1, ..., v_{n-1}] → n new quad faces.
    // For vertex v_i in the old face, the new quad is:
    //   [new_v_idx[v_i],  ep(v_i → v_{i+1}),  face_point,  ep(v_{i-1} → v_i)]
    // Winding: CCW is preserved because we go vertex → next_edge → center → prev_edge.
    for (fi, face) in mesh.faces.iter().enumerate() {
        let n = face.len();
        for i in 0..n {
            let vi_curr = face[i];
            let vi_next = face[(i + 1) % n];
            let vi_prev = face[(i + n - 1) % n];

            let ep_next = edge_map[&edge_key(vi_curr, vi_next)].new_idx;
            let ep_prev = edge_map[&edge_key(vi_prev, vi_curr)].new_idx;

            out.add_face(vec![
                new_v_idx[vi_curr],
                ep_next,
                face_point_idx[fi],
                ep_prev,
            ]);
        }
    }

    out
}

/// Apply Catmull-Clark subdivision `levels` times.
/// `levels = 0` returns a clone of the input mesh.
///
/// Output vertex counts (starting from a cube with 8 verts, 6 faces):
///   levels=0:  8 verts,  6 quad faces
///   levels=1: 26 verts, 24 quad faces
///   levels=2: 98 verts, 96 quad faces  ← test scene uses this
///   levels=3: 386 verts, 384 quad faces
pub fn subdivide(mesh: &PolyMesh, levels: u32) -> PolyMesh {
    if levels == 0 {
        return PolyMesh {
            positions: mesh.positions.clone(),
            faces: mesh.faces.clone(),
        };
    }

    // Apply CC levels times, reusing the intermediate PolyMesh each time
    let mut current = PolyMesh {
        positions: mesh.positions.clone(),
        faces: mesh.faces.clone(),
    };
    for _ in 0..levels {
        current = catmull_clark(&current);
    }
    current
}
