# Procedural Modeling: Skin Modifier + Catmull-Clark Subdivision

**Date:** 2026-02-18
**Status:** Decided — pipeline implemented and tested

---

## Problem Statement

All units and buildings use procedurally generated meshes rather than hand-authored assets. We need a system that converts a compact **vertex graph** (10–50 nodes per unit) into a smooth rendered mesh, mirroring Blender's Skin Modifier + Subdivision Surface workflow.

**Requirements:**
- Compact input (store 10–50 nodes, not thousands of triangles)
- Smooth, organic output (subdivision levels 0–3)
- LOD support (vary subdivision level with camera distance)
- Animation-friendly (keyframe source vertices, regenerate mesh per frame or cache frames)

---

## Pipeline

```
SkinGraph (vertices with radii, edges)
  → skin_modifier()       → PolyMesh (quad faces)
  → catmull_clark() × N  → PolyMesh (quad faces, smoother)
  → triangulate_smooth() → RenderMesh (GpuVertex + u32 indices)
  → wgpu buffers         → GPU (1 draw call per mesh type, instanced)
```

---

## Decision 1: Catmull-Clark vs Loop Subdivision

| | Catmull-Clark | Loop |
|---|---|---|
| Input | Any n-gon | Triangles only |
| Output | All-quad | All-triangle |
| Smoothness | C2 interior, C1 at extraordinary points | C2 interior |
| Complexity | Moderate | Moderate |

**Decision: Catmull-Clark**

The skin modifier naturally produces quad faces (tubes are generated as quad strips, isolated vertices produce cube quads). Catmull-Clark works directly on these without a triangulation step. Loop would require triangulating the skin output first, losing the clean quad topology that Catmull-Clark converges on efficiently.

---

## Decision 2: Mesh Representation — PolyMesh vs Half-Edge

| | PolyMesh + HashMap | Half-Edge |
|---|---|---|
| Topology queries | O(V+E+F) build, O(1) lookup | O(1) all queries |
| Rust complexity | Simple owned Vecs + HashMap | Self-referential structs (difficult) |
| Memory | `Vec<Vec<usize>>` per face | Compact but complex |
| Performance | Startup-only — not critical | Overkill |

**Decision: PolyMesh + HashMap adjacency**

Mesh generation runs **once at startup** (or on unit definition load). Half-edge's performance advantage is irrelevant here. PolyMesh uses straightforward owned data — no raw pointers, no arena allocators, no lifetime gymnastics. The `HashMap<(usize,usize), EdgeEntry>` edge map provides the adjacency needed for Catmull-Clark efficiently enough.

---

## Decision 3: Smooth vs Flat Normals

| | Smooth | Flat |
|---|---|---|
| GpuVertex count (level 2) | **98** (shared) | 576 (no sharing) |
| Vertex cache efficiency | High — avg ~6 tris/vertex | None |
| Vertex shader invocations (8 entities) | ~784 | ~4608 |
| Visual style | Smooth curved surface | Faceted polygons |

**Decision: Smooth normals**

Smooth normals are ~6× more vertex-efficient for level-2 subdivision. At 500+ units on screen, the difference in vertex bandwidth compounds. The visual style (organic curves matching the "Planetary Annihilation + slightly more organic" design brief) also favors smooth normals. Flat normals are an option if a deliberately faceted aesthetic is desired for specific unit types.

**Algorithm:** Area-weighted averaging. Cross product magnitude = 2×triangle area, so summing unnormalized cross products before normalizing automatically weights each face by its area.

---

## Catmull-Clark Algorithm

**Reference:** Catmull, E.; Clark, J. (1978). "Recursively generated B-spline surfaces on arbitrary topological meshes." Computer-Aided Design, 10(6), 350–355.

### One subdivision level: 4 phases

**Phase 0 — Build adjacency:**
- `vertex_faces[v]` = list of face indices adjacent to vertex v
- `vertex_edges[v]` = list of canonical edge keys `(min,max)` incident to v
- `edge_map: HashMap<(usize,usize), EdgeEntry>` with adjacent faces recorded

**Phase 1 — Face centroids:**
```
face_centroid[f] = average of positions of face[f].vertices
```

**Phase 2 — Edge points (added to output mesh):**
```
interior: ep = (pos[a] + pos[b] + centroid[f0] + centroid[f1]) / 4
boundary: ep = (pos[a] + pos[b]) / 2
```

**Phase 3 — Updated original vertices:**
```
n = number of adjacent faces (valence)
F = average of adjacent face centroids
R = average of midpoints of adjacent edges
new_V = (F + 2R + (n-3) × V) / n
```

**Phase 4 — Reconstruct faces:**
Each old n-gon face → n new quad faces. For vertex v_i in old face f:
```
new_quad = [new_V[v_i], ep(v_i → v_{i+1}), face_point[f], ep(v_{i-1} → v_i)]
```

### Vertex count formula (closed all-quad mesh)

```
V_new = V + E + F     (edges and faces each contribute one new point)
E_new = 2E + 4F
F_new = 4F
```

### Verification (starting from cube: 8 verts, 6 faces, 12 edges)

| Level | Verts | Quad Faces | Smooth GpuVertex | u32 Indices |
|-------|-------|------------|-----------------|-------------|
| 0     | 8     | 6          | 8               | 36          |
| 1     | 26    | 24         | 26              | 144         |
| 2     | 98    | 96         | 98              | 576         |
| 3     | 386   | 384        | 386             | 2304        |

Level 2 check: V=26 + E=48 + F=24 = 98 ✓

---

## Skin Modifier

### Isolated vertex (degree 0) → cube

For a node at position `p` with radius `r`, an axis-aligned cube with half-extent `r`.
8 vertices, 6 quad faces, CCW winding from outside.

This matches Blender's skin modifier behavior for isolated nodes. Catmull-Clark converges a cube to a sphere-like shape — after 2 levels the mesh is visually smooth and round.

Winding verified: front face `[0,1,2,3]` → N = (v1-v0)×(v3-v0) = +Z (outward) ✓

### Edge case (future) — tube generation

For an edge between nodes A and B:
1. Compute the edge direction vector `dir = normalize(B.pos - A.pos)`
2. Build a perpendicular frame using Gram-Schmidt orthogonalization
3. Generate N-gon cross-sections at A and B, radius = node.radius
4. Connect with N quad faces (quad strip)
5. Add hemisphere/flat caps at degree-1 endpoints
6. Merge cross-sections at degree-3+ junctions (complex topology)

---

## LOD Strategy

Subdivision level is varied by camera distance:

| Distance | CC Levels | Approximate Tris |
|----------|-----------|-----------------|
| 0–10u (near)    | 3 | 768 |
| 10–30u (medium) | 2 | 192 |
| 30–60u (far)    | 1 | 48  |
| 60u+ (distant)  | 0 | 12  |

For the test scene and initial implementation, level 2 is used for all entities.
LOD switching will be added once unit selection and gameplay systems are in place.

---

## Implementation Notes

- `PolyMesh` uses `Vec<Vec<usize>>` for faces — heap allocation per face is acceptable at startup
- Edge canonicalization: `edge_key(a, b) = (min(a,b), max(a,b))` ensures `(a,b)` and `(b,a)` map to the same entry
- `normalize_or_zero()` used for normal normalization — safe against zero-length normals (impossible in a well-formed closed mesh, but defensive)
- Index type: `u32` — future complex skin graphs or high subdivision levels could exceed 65535 vertices; u32 future-proofs this
- The existing `shader_instanced.wgsl` requires no changes — smooth normals work correctly with the existing Blinn-Phong fragment shader

---

## Test Scene

Single vertex at origin, radius 0.5 → skin modifier → cube → CC×2 → near-sphere.
8 entities in a 2×2×2 arrangement, static (no `Velocity` component), each a different color.
Demonstrates the full pipeline and verifies correct lighting on the smooth subdivided surface.
