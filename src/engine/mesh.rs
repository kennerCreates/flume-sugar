// Procedural mesh types and triangulation.
// See docs/research/procedural-modeling.md for algorithm decisions.
//
// Three-layer architecture:
//   SkinGraph → skin_modifier() → PolyMesh → catmull_clark() → PolyMesh → triangulate_smooth() → RenderMesh → GPU

use glam::Vec3;

// ============================================================================
// GPU VERTEX
// ============================================================================

/// GPU-ready vertex with position and normal.
/// Byte layout is identical to the old `Vertex` struct in main.rs:
///   @location(0) position: vec3<f32>
///   @location(1) normal:   vec3<f32>
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVertex {
    pub position: [f32; 3],
    pub normal:   [f32; 3],
}

impl GpuVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

// ============================================================================
// POLY MESH
// ============================================================================

/// Intermediate polygon mesh for procedural manipulation.
/// Supports n-gon faces (arbitrary vertex count per face).
/// Faces use CCW winding when viewed from outside (consistent with back-face culling).
/// NOT GPU-ready — use `RenderMesh` for rendering.
/// Only used at startup/load time; heap allocation per face is acceptable.
pub struct PolyMesh {
    pub positions: Vec<Vec3>,
    pub faces:     Vec<Vec<usize>>,  // each face = CCW-ordered vertex index list
}

impl PolyMesh {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            faces:     Vec::new(),
        }
    }

    /// Add a vertex and return its index.
    pub fn add_vertex(&mut self, pos: Vec3) -> usize {
        let idx = self.positions.len();
        self.positions.push(pos);
        idx
    }

    /// Add a face by vertex indices (CCW order).
    pub fn add_face(&mut self, indices: Vec<usize>) {
        debug_assert!(indices.len() >= 3, "Face must have at least 3 vertices");
        self.faces.push(indices);
    }

    pub fn vertex_count(&self) -> usize { self.positions.len() }
}

// ============================================================================
// RENDER MESH
// ============================================================================

/// GPU-ready triangulated mesh with per-vertex normals.
/// Vertices are shared across triangles via the index buffer (smooth normals).
/// Upload vertex_bytes() to a VERTEX buffer, index_bytes() to an INDEX buffer.
pub struct RenderMesh {
    pub vertices: Vec<GpuVertex>,
    pub indices:  Vec<u32>,
}

impl RenderMesh {
    /// Cast vertex slice to raw bytes for wgpu buffer upload.
    pub fn vertex_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.vertices)
    }

    /// Cast index slice to raw bytes for wgpu buffer upload.
    pub fn index_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.indices)
    }

    pub fn index_count(&self) -> usize  { self.indices.len() }
}

// ============================================================================
// TRIANGULATION + SMOOTH NORMALS
// ============================================================================

/// Convert a PolyMesh to a GPU-ready RenderMesh using smooth (area-weighted) normals.
///
/// Smooth normals: vertices are shared across triangles via the index buffer.
/// For a CC-subdivided mesh with 98 positions → 98 GpuVertex (not 576).
///
/// Algorithm:
///   1. Accumulate area-weighted face normals into each vertex's normal accumulator.
///      The cross product magnitude = 2×triangle_area, giving automatic area-weighting.
///   2. Normalize each accumulated normal.
///   3. Fan-triangulate each face (from vertex 0).
///   4. Build the index buffer referencing shared GpuVertex entries.
pub fn triangulate_smooth(poly: &PolyMesh) -> RenderMesh {
    let n_verts = poly.vertex_count();

    // Step 1: Accumulate area-weighted normals per vertex
    let mut normal_accum: Vec<Vec3> = vec![Vec3::ZERO; n_verts];

    for face in &poly.faces {
        let n = face.len();
        // Fan triangulate from vertex 0
        for i in 1..(n - 1) {
            let a = poly.positions[face[0]];
            let b = poly.positions[face[i]];
            let c = poly.positions[face[i + 1]];
            // Cross product is not normalized — magnitude encodes 2×area (area-weighting)
            let weighted_normal = (b - a).cross(c - a);
            normal_accum[face[0]]     += weighted_normal;
            normal_accum[face[i]]     += weighted_normal;
            normal_accum[face[i + 1]] += weighted_normal;
        }
    }

    // Step 2: Build GpuVertex per PolyMesh position
    let vertices: Vec<GpuVertex> = poly.positions.iter()
        .zip(normal_accum.iter())
        .map(|(pos, n)| GpuVertex {
            position: pos.to_array(),
            normal:   n.normalize_or_zero().to_array(),
        })
        .collect();

    // Step 3 & 4: Fan-triangulate faces to build index buffer
    let mut indices: Vec<u32> = Vec::new();
    for face in &poly.faces {
        let n = face.len();
        for i in 1..(n - 1) {
            indices.push(face[0]     as u32);
            indices.push(face[i]     as u32);
            indices.push(face[i + 1] as u32);
        }
    }

    RenderMesh { vertices, indices }
}
