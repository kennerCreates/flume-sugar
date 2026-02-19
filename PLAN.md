# Implementation Plan: Procedural Modeling System (Skin + Catmull-Clark)

## Goal

Replace the 1000 bouncing cubes test scene with a **procedural modeling pipeline** mirroring
Blender's Skin Modifier + Subdivision Surface workflow. The test scene shows a mesh generated from
a **single vertex** → skin modifier (cube) → Catmull-Clark ×2 (near-sphere, 96 quads) →
smooth-normal triangulation. This validates the full pipeline before building real unit models.

---

## What We're Building

### The Pipeline
```
SkinGraph (1 vertex, r=0.5)
  → skin_modifier()         → PolyMesh (8 verts, 6 quad faces — a cube)
  → catmull_clark() × 2    → PolyMesh (98 verts, 96 quad faces — near-sphere)
  → triangulate_smooth()   → RenderMesh (98 GpuVertex, 576 u32 indices)
  → wgpu buffers           → GPU
```

### Why Smooth Normals
Vertices are **shared** across triangles via the index buffer — the GPU caches and reuses them.

| Normal Style | GpuVertex | Indices | Notes |
|---|---|---|---|
| **Smooth (chosen)** | **98** | **576** | ~6× fewer vertex shader invocations |
| Flat | 576 | 576 | No sharing; each triangle has 3 unique verts |

At 500+ units with complex meshes, the vertex count difference compounds significantly. Smooth also gives correct Blinn-Phong specular on curved surfaces.

### Mesh Counts by Subdivision Level
| Level | PolyMesh Verts | Quad Faces | Smooth GpuVertex | Indices |
|-------|---------------|------------|-----------------|---------|
| 0 (cube)  | 8    | 6   | 8    | 36   |
| 1         | 26   | 24  | 26   | 144  |
| **2** ← test | **98** | **96** | **98** | **576** |
| 3         | 386  | 384 | 386  | 2304 |

Formula: `V_new = V + E + F` (for closed all-quad mesh).
Level 2: `V=26 + E=48 + F=24 = 98` ✓

### The Test Scene
- 8 sphere-like entities in a 2×2×2 cube arrangement (3 units apart, centered ~y=3.5)
- Static (no `Velocity` component) — clearly inspect geometry from all angles
- Each entity a different color via existing `InstanceData`
- Existing instanced rendering unchanged — 1 draw call for all 8
- Existing `shader_instanced.wgsl` works unchanged — smooth normals → correct Blinn-Phong

---

## Files to Create

### 1. `src/engine/mesh.rs` — Core polygon mesh types

```rust
// GPU-ready vertex — byte-identical to existing `Vertex` in main.rs (same shader locations)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVertex {
    pub position: [f32; 3],   // @location(0)
    pub normal:   [f32; 3],   // @location(1)
}

impl GpuVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static>  // identical to existing Vertex::desc()
}

// Intermediate polygon mesh — n-gon faces, heap-allocated, not GPU-ready
pub struct PolyMesh {
    pub positions: Vec<Vec3>,
    pub faces:     Vec<Vec<usize>>,  // each face = CCW-ordered vertex index list
}

impl PolyMesh {
    pub fn new() -> Self
    pub fn add_vertex(&mut self, pos: Vec3) -> usize
    pub fn add_face(&mut self, indices: Vec<usize>)
    pub fn vertex_count(&self) -> usize
    pub fn face_count(&self) -> usize
}

// Triangulated, smooth-normal, GPU-ready mesh
pub struct RenderMesh {
    pub vertices: Vec<GpuVertex>,
    pub indices:  Vec<u32>,
}

impl RenderMesh {
    pub fn vertex_bytes(&self) -> &[u8]   // bytemuck::cast_slice(&self.vertices)
    pub fn index_bytes(&self) -> &[u8]    // bytemuck::cast_slice(&self.indices)
    pub fn vertex_count(&self) -> usize
    pub fn index_count(&self) -> usize
}

// Entry point: fan-triangulate + compute area-weighted smooth normals
pub fn triangulate_smooth(poly: &PolyMesh) -> RenderMesh
```

**`triangulate_smooth` algorithm:**

```
// 1. Accumulate area-weighted normals per vertex
normal_accum: Vec<Vec3> = vec![Vec3::ZERO; poly.vertex_count()]

for face in &poly.faces:
    // Fan-triangulate from v0
    for i in 1..(face.len() - 1):
        A = positions[face[0]]
        B = positions[face[i]]
        C = positions[face[i+1]]
        // Cross product magnitude = 2×area (area-weighting is automatic)
        weighted_normal = (B - A).cross(C - A)
        normal_accum[face[0]]   += weighted_normal
        normal_accum[face[i]]   += weighted_normal
        normal_accum[face[i+1]] += weighted_normal

// 2. Build GpuVertex per PolyMesh position (vertices are shared via index buffer)
vertices: Vec<GpuVertex> = poly.positions.iter().zip(normal_accum.iter())
    .map(|(pos, n)| GpuVertex {
        position: pos.to_array(),
        normal: n.normalize_or_zero().to_array(),
    })
    .collect()

// 3. Build index buffer (fan triangulation — same face loop as above)
indices: Vec<u32> = []
for face in &poly.faces:
    for i in 1..(face.len() - 1):
        indices.push(face[0] as u32)
        indices.push(face[i] as u32)
        indices.push(face[i+1] as u32)

return RenderMesh { vertices, indices }
```

No vertex duplication. Each of the 98 PolyMesh positions becomes exactly 1 GpuVertex.
The 576 index entries reference those 98 vertices, with heavy reuse (avg ~6 triangles per vertex).

---

### 2. `src/engine/skin.rs` — Skin modifier

```rust
pub struct SkinNode { pub position: Vec3, pub radius: f32 }
pub struct SkinEdge { pub a: usize, pub b: usize }

pub struct SkinGraph {
    pub nodes: Vec<SkinNode>,
    pub edges: Vec<SkinEdge>,
}
impl SkinGraph {
    pub fn new() -> Self
    pub fn add_node(&mut self, position: Vec3, radius: f32) -> usize
    pub fn add_edge(&mut self, a: usize, b: usize)
    fn degree(&self, node_idx: usize) -> usize
}

pub fn skin_modifier(graph: &SkinGraph) -> PolyMesh
```

**Isolated vertex (degree 0) → cube:**

For a node at position `p` with radius `r`, 8 vertices and 6 CCW quad faces:

```
Vertices:
  0: p + (-r, -r, +r)   front-bottom-left
  1: p + (+r, -r, +r)   front-bottom-right
  2: p + (+r, +r, +r)   front-top-right
  3: p + (-r, +r, +r)   front-top-left
  4: p + (+r, -r, -r)   back-bottom-right
  5: p + (-r, -r, -r)   back-bottom-left
  6: p + (-r, +r, -r)   back-top-left
  7: p + (+r, +r, -r)   back-top-right

Faces (CCW from outside):
  front (+Z): [0, 1, 2, 3]
  back  (-Z): [4, 5, 6, 7]
  left  (-X): [5, 0, 3, 6]
  right (+X): [1, 4, 7, 2]
  top   (+Y): [3, 2, 7, 6]
  bottom(-Y): [5, 4, 1, 0]
```

Winding verified: front face [0,1,2,3] → N = (v1-v0)×(v3-v0) = +Z (outward) ✓

**Edge handling:** `skin_modifier` iterates `graph.edges`. For degree-0 nodes (no edges) it
generates the cube above. Edge tube generation is stubbed for now — add
`// TODO: edge tubes (cross-section rings via Gram-Schmidt)` with no panicking code,
since the test scene has no edges.

---

### 3. `src/engine/subdivide.rs` — Catmull-Clark subdivision

```rust
pub fn catmull_clark(mesh: &PolyMesh) -> PolyMesh
pub fn subdivide(mesh: &PolyMesh, levels: u32) -> PolyMesh  // calls CC `levels` times
```

**Internal types:**
```rust
fn edge_key(a: usize, b: usize) -> (usize, usize)  // always (min, max)

struct EdgeEntry {
    adjacent_faces: Vec<usize>,  // 1 = boundary, 2 = interior
    new_idx: usize,              // edge point index in output mesh, set during phase 2
}
type EdgeMap = std::collections::HashMap<(usize, usize), EdgeEntry>;
```

**Algorithm — 4 phases:**

**Phase 0 — Build adjacency** (single pass over all faces):
```rust
let mut vertex_faces: Vec<Vec<usize>> = vec![vec![]; n_verts];
let mut vertex_edges: Vec<Vec<(usize, usize)>> = vec![vec![]; n_verts];
let mut edge_map: EdgeMap = HashMap::new();

for (fi, face) in mesh.faces.iter().enumerate() {
    let n = face.len();
    for (i, &vi) in face.iter().enumerate() {
        vertex_faces[vi].push(fi);
        let vj = face[(i + 1) % n];
        let key = edge_key(vi, vj);
        let entry = edge_map.entry(key).or_insert(EdgeEntry { adjacent_faces: vec![], new_idx: 0 });
        if !entry.adjacent_faces.contains(&fi) {
            entry.adjacent_faces.push(fi);
        }
        if !vertex_edges[vi].contains(&key) { vertex_edges[vi].push(key); }
        if !vertex_edges[vj].contains(&key) { vertex_edges[vj].push(key); }
    }
}
```

**Phase 1 — Face centroids** (stored as `Vec<Vec3>`, not yet added to new mesh):
```rust
let face_centroids: Vec<Vec3> = mesh.faces.iter().map(|face| {
    face.iter().map(|&vi| mesh.positions[vi]).sum::<Vec3>() / face.len() as f32
}).collect();
```

**Phase 2 — Edge points** (added to `out` mesh, new indices recorded in EdgeEntry):
```rust
for entry in edge_map.values_mut() {
    let (a, b) = /* key */;
    let pa = mesh.positions[a];
    let pb = mesh.positions[b];
    let ep = if entry.adjacent_faces.len() == 2 {
        (pa + pb + face_centroids[entry.adjacent_faces[0]] + face_centroids[entry.adjacent_faces[1]]) / 4.0
    } else {
        (pa + pb) / 2.0  // boundary
    };
    entry.new_idx = out.add_vertex(ep);
}
```

**Phase 3 — Updated original vertices**:
```rust
let new_v_idx: Vec<usize> = (0..n_verts).map(|v| {
    let adj = &vertex_faces[v];
    let n = adj.len() as f32;
    let f = adj.iter().map(|&fi| face_centroids[fi]).sum::<Vec3>() / n;
    let r = vertex_edges[v].iter()
        .map(|&(a, b)| (mesh.positions[a] + mesh.positions[b]) / 2.0)
        .sum::<Vec3>() / n;
    let new_pos = (f + 2.0 * r + (n - 3.0) * mesh.positions[v]) / n;
    out.add_vertex(new_pos)
}).collect();
```

**Phase 1b — Face points** (added to `out` mesh after original vertices):
```rust
let face_point_idx: Vec<usize> = face_centroids.iter()
    .map(|&c| out.add_vertex(c))
    .collect();
```

**Phase 4 — Reconstruct faces:**
Each old n-gon face → n new quads:
```rust
for (fi, face) in mesh.faces.iter().enumerate() {
    let n = face.len();
    for i in 0..n {
        let vi_curr = face[i];
        let vi_next = face[(i + 1) % n];
        let vi_prev = face[(i + n - 1) % n];
        out.add_face(vec![
            new_v_idx[vi_curr],
            edge_map[&edge_key(vi_curr, vi_next)].new_idx,
            face_point_idx[fi],
            edge_map[&edge_key(vi_prev, vi_curr)].new_idx,
        ]);
    }
}
```

**Winding verification:** For a CCW quad face `[A,B,C,D]`, vertex A's new quad is
`[new_A, ep(AB), fp, ep(DA)]`. Going around this quad, the face normal still points outward. ✓

---

### 4. `docs/research/procedural-modeling.md` — Research document

Covers:
- **Problem**: Generating organic meshes from skeletal graphs at scale (500+ units)
- **Pipeline overview** with diagram
- **Algorithm choice: Catmull-Clark vs Loop** — CC operates on n-gons natively, outputs quads; Loop requires triangles. Skin modifier outputs quads. CC chosen.
- **Mesh representation: PolyMesh + HashMap vs Half-Edge** — Half-edge requires self-referential structs in Rust (difficult ownership). HashMap adjacency is O(V+E+F) build, O(1) lookup, and sufficient for startup-only generation. Decision: PolyMesh.
- **Normal style: Smooth vs Flat** — Smooth chosen: 98 vertices vs 576 for level 2; ~6× fewer vertex invocations; scales with unit count; correct Blinn-Phong on curved surfaces.
- **Subdivision level 2** for test scene — near-sphere at 98 verts / 192 tris. LOD plan: 0 (distant) = 0 levels, 1 (far) = 1 level, 2 (medium) = 2 levels, 3 (near) = 3 levels.
- **Cube for isolated vertex** — matches Blender behavior; CC converges to sphere.
- **Reference**: Catmull & Clark (1978), CAD journal.
- **Vertex count formula**: `V_new = V + E + F` for closed all-quad mesh. Verification table.
- **Future work**: Edge tubes (Gram-Schmidt cross-sections), junction topology, crease weights.

---

## Files to Modify

### 5. `src/engine/mod.rs`
```rust
pub mod mesh;
pub mod skin;
pub mod subdivide;

pub use mesh::{PolyMesh, RenderMesh, GpuVertex};
pub use skin::SkinGraph;
pub use subdivide::subdivide;
```

---

### 6. `src/main.rs`

**Remove:**
- `struct Vertex` and `impl Vertex { fn desc() }` → use `engine::mesh::GpuVertex` and `GpuVertex::desc()`
- `const CUBE_VERTICES` and `const CUBE_INDICES`
- `fn spawn_test_entities()`
- Movement + bounds systems in `State::update()` (entities have no `Velocity`; the queries match zero entities so they do nothing, but leaving the dead code causes warnings about `bounds`, `half_bounds`, unused `rand::Rng` import)
- `use rand::Rng` import

**Add:**
```rust
fn build_procedural_sphere() -> engine::mesh::RenderMesh {
    use engine::{skin::SkinGraph, subdivide::subdivide, mesh::triangulate_smooth};
    let mut graph = SkinGraph::new();
    graph.add_node(glam::Vec3::ZERO, 0.5);
    let cube = engine::skin::skin_modifier(&graph);  // 8 verts, 6 faces
    let subd = subdivide(&cube, 2);                  // 98 verts, 96 faces
    let mesh = triangulate_smooth(&subd);            // 98 GpuVertex, 576 indices
    println!("Procedural mesh: {} verts, {} indices ({} tris)",
        mesh.vertex_count(), mesh.index_count(), mesh.index_count() / 3);
    mesh
}

fn spawn_procedural_test_scene(world: &mut World) {
    // 8 static entities — no Velocity, so bounds/movement queries match nothing
    // 2×2×2 cube arrangement, spacing=3.0, y centered at 3.5
    let positions = [
        Vec3::new(-1.5, 2.0, -1.5), Vec3::new( 1.5, 2.0, -1.5),
        Vec3::new(-1.5, 2.0,  1.5), Vec3::new( 1.5, 2.0,  1.5),
        Vec3::new(-1.5, 5.0, -1.5), Vec3::new( 1.5, 5.0, -1.5),
        Vec3::new(-1.5, 5.0,  1.5), Vec3::new( 1.5, 5.0,  1.5),
    ];
    let colors = [
        Color { r: 1.0, g: 0.3, b: 0.3 },  // red
        Color { r: 0.3, g: 1.0, b: 0.3 },  // green
        Color { r: 0.3, g: 0.3, b: 1.0 },  // blue
        Color { r: 1.0, g: 1.0, b: 0.3 },  // yellow
        Color { r: 1.0, g: 0.3, b: 1.0 },  // magenta
        Color { r: 0.3, g: 1.0, b: 1.0 },  // cyan
        Color { r: 1.0, g: 0.6, b: 0.2 },  // orange
        Color { r: 0.8, g: 0.8, b: 0.8 },  // white
    ];
    for (pos, color) in positions.iter().zip(colors.iter()) {
        world.spawn((Transform::from_position(*pos), *color));
        // Note: no Velocity — static test scene
    }
    println!("Spawned 8 procedural sphere entities");
}
```

**In `State::new()`:** Replace cube buffer creation block:
```rust
let render_mesh = build_procedural_sphere();
let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: Some("Procedural Vertex Buffer"),
    contents: render_mesh.vertex_bytes(),
    usage: wgpu::BufferUsages::VERTEX,
});
let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: Some("Procedural Index Buffer"),
    contents: render_mesh.index_bytes(),
    usage: wgpu::BufferUsages::INDEX,
});
let num_indices = render_mesh.index_count() as u32;
```

Replace `spawn_test_entities(&mut world, 1000)` with `spawn_procedural_test_scene(&mut world)`.

**In `State::render()`:** One-line change:
```rust
// Before: wgpu::IndexFormat::Uint16
// After:
render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
```

**Everywhere `Vertex::desc()` is referenced:** Replace with `GpuVertex::desc()` (same return value).

---

### 7. `CLAUDE.md`
Update module structure table, replace cube geometry description with procedural pipeline, note `GpuVertex` as canonical vertex type.

### 8. `docs/NEXT_STEPS.md`
Change `procedural-modeling.md` row from `TODO` → `Done`.

---

## Implementation Order

1. `src/engine/mesh.rs` — `PolyMesh`, `RenderMesh`, `GpuVertex`, `triangulate_smooth()`
2. `src/engine/skin.rs` — `SkinGraph` + `skin_modifier()` (degree-0 cube only)
3. `src/engine/subdivide.rs` — `catmull_clark()` + `subdivide()`
4. `src/engine/mod.rs` — `pub mod` declarations + re-exports
5. `src/main.rs` — wire pipeline, remove old code, verify zero-warning build
6. `docs/research/procedural-modeling.md` — write research doc
7. `CLAUDE.md` + `NEXT_STEPS.md` — update

---

## Key Invariants

| Invariant | How enforced |
|-----------|--------------|
| Zero-warning build | Remove all dead code; no `#[allow(...)]` |
| CCW winding preserved | Cube faces CCW → CC phases preserve winding → triangulation inherits CCW → normals outward |
| `GpuVertex` byte layout | `[f32;3] + [f32;3]` = identical to old `Vertex`; pipeline descriptor unchanged |
| Shader unchanged | `shader_instanced.wgsl` works with smooth normals and existing locations 0/1/2/3 |
| Instanced rendering unchanged | `InstanceData` provides position+color per entity; 1 draw call |
| No new dependencies | `glam`, `bytemuck`, `wgpu`, `bevy_ecs`, `std::collections::HashMap` — all sufficient |
