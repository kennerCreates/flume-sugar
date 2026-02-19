// Skin modifier: converts a SkinGraph (vertex graph with radii) into a PolyMesh.
// Mirrors Blender's Skin Modifier workflow.
// See docs/research/procedural-modeling.md for algorithm decisions.
//
// Supported cases:
//   degree 0 (isolated vertex) → axis-aligned cube with CCW quad faces
//
// Future:
//   degree 1 (chain end)       → tube + hemisphere cap
//   degree 2 (chain through)   → smooth tube pass-through
//   degree 3+ (junction)       → cross-section merge (Gram-Schmidt frame + stitching)

use glam::Vec3;
use super::mesh::PolyMesh;

// ============================================================================
// SKIN GRAPH
// ============================================================================

/// A node in the skin graph: a 3D point with a radius.
/// Radius controls the cross-section size of the geometry generated around this node.
pub struct SkinNode {
    pub position: Vec3,
    pub radius:   f32,
}

/// An undirected edge connecting two nodes in the skeleton.
pub struct SkinEdge {
    pub a: usize,  // index into SkinGraph::nodes
    pub b: usize,
}

/// Input graph for the Skin Modifier.
/// Nodes are joints, edges are bones. Typically 10–50 nodes per unit.
pub struct SkinGraph {
    pub nodes: Vec<SkinNode>,
    pub edges: Vec<SkinEdge>,
}

impl SkinGraph {
    pub fn new() -> Self {
        Self { nodes: Vec::new(), edges: Vec::new() }
    }

    /// Add a node at the given position with the given radius. Returns its index.
    pub fn add_node(&mut self, position: Vec3, radius: f32) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(SkinNode { position, radius });
        idx
    }

    /// Number of edges incident to the given node.
    fn degree(&self, node_idx: usize) -> usize {
        self.edges.iter()
            .filter(|e| e.a == node_idx || e.b == node_idx)
            .count()
    }
}

// ============================================================================
// SKIN MODIFIER
// ============================================================================

/// Convert a SkinGraph into a PolyMesh.
/// Currently handles isolated vertices (degree 0) → cube.
/// Edge tubes (degree 1+) are not yet implemented.
pub fn skin_modifier(graph: &SkinGraph) -> PolyMesh {
    let mut mesh = PolyMesh::new();

    for (idx, node) in graph.nodes.iter().enumerate() {
        match graph.degree(idx) {
            0 => skin_isolated_vertex(node, &mut mesh),
            // TODO: edge tubes via cross-section rings (Gram-Schmidt frame perpendicular to edge)
            _ => { /* edges handled separately below once tube generation is implemented */ }
        }
    }

    // TODO: for each edge, generate a quad tube connecting cross-sections at each endpoint.
    // For now, edges are silently skipped — the test scene has no edges.
    let _ = &graph.edges;

    mesh
}

// ============================================================================
// DEGREE-0: ISOLATED VERTEX → CUBE
// ============================================================================

/// Generate a cube PolyMesh around an isolated node (no edges).
/// The cube has half-extent = node.radius on each axis, centered on node.position.
/// All 6 faces use CCW winding viewed from outside (consistent with back-face culling).
///
/// Vertex layout (relative offsets from center p, half-extent r):
///   0: (-r, -r, +r)  front-bottom-left
///   1: (+r, -r, +r)  front-bottom-right
///   2: (+r, +r, +r)  front-top-right
///   3: (-r, +r, +r)  front-top-left
///   4: (+r, -r, -r)  back-bottom-right
///   5: (-r, -r, -r)  back-bottom-left
///   6: (-r, +r, -r)  back-top-left
///   7: (+r, +r, -r)  back-top-right
///
/// Winding verification (front face [0,1,2,3]):
///   N = (v1-v0) × (v3-v0) = (2r,0,0) × (0,2r,0) = (0,0,4r²) → +Z (outward) ✓
fn skin_isolated_vertex(node: &SkinNode, mesh: &mut PolyMesh) {
    let p = node.position;
    let r = node.radius;

    // The 8 vertices of the axis-aligned cube
    let base = mesh.vertex_count();
    mesh.add_vertex(Vec3::new(p.x - r, p.y - r, p.z + r)); // 0 front-bottom-left
    mesh.add_vertex(Vec3::new(p.x + r, p.y - r, p.z + r)); // 1 front-bottom-right
    mesh.add_vertex(Vec3::new(p.x + r, p.y + r, p.z + r)); // 2 front-top-right
    mesh.add_vertex(Vec3::new(p.x - r, p.y + r, p.z + r)); // 3 front-top-left
    mesh.add_vertex(Vec3::new(p.x + r, p.y - r, p.z - r)); // 4 back-bottom-right
    mesh.add_vertex(Vec3::new(p.x - r, p.y - r, p.z - r)); // 5 back-bottom-left
    mesh.add_vertex(Vec3::new(p.x - r, p.y + r, p.z - r)); // 6 back-top-left
    mesh.add_vertex(Vec3::new(p.x + r, p.y + r, p.z - r)); // 7 back-top-right

    // 6 quad faces, CCW winding from outside
    let v = |i: usize| base + i;
    mesh.add_face(vec![v(0), v(1), v(2), v(3)]); // front  (+Z)
    mesh.add_face(vec![v(4), v(5), v(6), v(7)]); // back   (-Z)
    mesh.add_face(vec![v(5), v(0), v(3), v(6)]); // left   (-X)
    mesh.add_face(vec![v(1), v(4), v(7), v(2)]); // right  (+X)
    mesh.add_face(vec![v(3), v(2), v(7), v(6)]); // top    (+Y)
    mesh.add_face(vec![v(5), v(4), v(1), v(0)]); // bottom (-Y)
}
