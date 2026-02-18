# Lighting Implementation Research

**Date:** 2026-02-17
**Decision:** Implement Blinn-Phong lighting with directional light for 1000-cube performance test

## Problem Statement

We need to add a lighting system to the 3D engine to:
1. Make the cubes look more visually appealing (currently flat colored)
2. Test performance impact of lighting calculations on 1000 moving cubes
3. Maintain our 1-draw-call instanced rendering efficiency

## Requirements

- Must work with existing instanced rendering (1000 entities, 1 draw call)
- Should maintain 60+ FPS performance
- Need to be simple enough to implement quickly
- Should be a foundation for future lighting enhancements

## Options Considered

### 1. Phong Lighting Model

**Pros:**
- Classic, well-documented approach
- Simple to understand conceptually

**Cons:**
- More expensive than Blinn-Phong (~2x slower)
- Requires computing reflection vector per-pixel
- Worse artifacts at glancing angles
- Not recommended for many dynamic objects

### 2. Blinn-Phong Lighting Model ✅ CHOSEN

**Pros:**
- More efficient than Phong for dynamic scenes
- Uses half-vector instead of reflection vector (computed once per light)
- Better visual results with fewer artifacts
- Industry standard for real-time applications
- Well-supported in wgpu tutorials

**Cons:**
- Slightly more complex math than basic Phong
- Not physically accurate (but we don't need PBR for this test)

**Why chosen:** Best balance of performance and quality for our use case.

### 3. PBR (Physically Based Rendering)

**Pros:**
- Most realistic lighting
- Physically accurate materials
- Industry standard for modern games

**Cons:**
- 3-5x more expensive than Blinn-Phong
- Complex to implement (Cook-Torrance BRDF, microfacet models)
- Overkill for a performance test with simple cubes
- Would significantly reduce maximum entity count

**Decision:** Too expensive for our current goals. Consider for future when adding complex materials.

## Light Type Comparison

### Directional Light ✅ CHOSEN

**Use case:** Sun, moon, distant light sources
**Performance:** Best (constant direction, no attenuation)
**Complexity:** Simplest to implement

**Characteristics:**
- Light direction is constant for all fragments
- No distance attenuation calculations needed
- Computed once per frame
- Perfect for testing performance with many objects

**Why chosen:** Simplest implementation, best performance, sufficient for initial testing.

### Point Light

**Use case:** Lamps, torches, explosions
**Performance:** Moderate (per-fragment direction calculation)
**Complexity:** Medium

**Characteristics:**
- Light direction varies per fragment (from light position to surface)
- Distance attenuation required
- More expensive for 1000 objects

**Future consideration:** Add after directional light works.

### Spot Light

**Use case:** Flashlights, stage lights
**Performance:** Worst (cone angle calculations + point light cost)
**Complexity:** Most complex

**Characteristics:**
- All point light calculations
- Plus: cone angle checks and falloff
- Requires inner/outer cone angles

**Future consideration:** Only if needed for specific effects.

## Implementation Approach

### Phase 1: Add Normal Vectors

Extend vertex data to include surface normals:

```rust
#[repr(C)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],    // NEW: surface normal for lighting
}
```

**Cube normals (per-face):**
- Front face: `[0.0, 0.0, 1.0]`
- Back face: `[0.0, 0.0, -1.0]`
- Right face: `[1.0, 0.0, 0.0]`
- Left face: `[-1.0, 0.0, 0.0]`
- Top face: `[0.0, 1.0, 0.0]`
- Bottom face: `[0.0, -1.0, 0.0]`

Each vertex on a face shares the same normal direction.

### Phase 2: Create Light Uniform

Define light data structure with proper WGSL alignment:

```rust
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniform {
    direction: [f32; 3],    // Light direction
    _padding: u32,           // CRITICAL: WGSL alignment (vec3 must align to 16 bytes)
    color: [f32; 3],         // Light color (RGB)
    _padding2: u32,          // CRITICAL: WGSL alignment
}
```

**Initial values:**
- Direction: `[-0.3, -0.5, -0.6]` (pointing down and to the side)
- Color: `[1.0, 1.0, 1.0]` (white light)

### Phase 3: Update Camera Uniform

Fragment shader needs camera position for specular calculations:

```rust
#[repr(C)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],     // NEW: camera world position
    _padding: u32,             // Alignment
}
```

### Phase 4: Update Shaders (Blinn-Phong)

**Vertex Shader:**
- Pass world position and normal to fragment shader
- No rotation on cubes, so normals don't need transformation

**Fragment Shader (three components):**

1. **Ambient:** `ambient = light_color * ambient_strength (0.1)`
2. **Diffuse:** `diffuse = light_color * max(dot(normal, light_dir), 0.0)`
3. **Specular:** `specular = light_color * pow(max(dot(normal, half_dir), 0.0), 32.0) * strength`

Where `half_dir = normalize(view_dir + light_dir)` (Blinn-Phong optimization)

**Final color:** `(ambient + diffuse + specular) * object_color`

## Performance Expectations

**Current performance:** ~60 FPS with 1000 cubes (no lighting)

**Expected with Blinn-Phong directional light:**
- FPS impact: 5-15% reduction
- Expected: 50-60 FPS with 1000 cubes
- Still maintains 1 draw call (instancing preserved)

**Cost breakdown:**
- Vertex shader: +negligible (just passing normals through)
- Fragment shader: +10-15 ALU operations per fragment
- Memory: +12 bytes per vertex (normals), +32 bytes (light uniform)
- Bandwidth: Minimal increase

**Why performance stays good:**
- Lighting is per-fragment, not per-instance
- Directional light has constant direction (no per-fragment calculation)
- Modern GPUs handle this math easily
- Instancing still batches all cubes in one draw call

**Optimization opportunities (if needed):**
1. Per-vertex lighting instead of per-fragment (faster but lower quality)
2. Reduce specular shininess calculation complexity
3. Use simplified shading for distant objects

## Critical Implementation Notes

### WGSL Alignment Requirements

**CRITICAL:** WGSL requires `vec3` fields in structs to be aligned to 16 bytes. Always add `_padding: u32` after `vec3` fields:

```rust
struct Wrong {
    direction: [f32; 3],  // ❌ Will cause GPU errors
    color: [f32; 3],       // ❌ Misaligned
}

struct Correct {
    direction: [f32; 3],
    _padding: u32,         // ✅ Align to 16 bytes
    color: [f32; 3],
    _padding2: u32,        // ✅ Align to 16 bytes
}
```

Failure to align causes GPU buffer access errors and undefined behavior.

### Normal Calculation for Moving Cubes

Our cubes currently only **translate** (no rotation). This means:
- Vertex normals can be used directly
- No normal matrix transformation needed
- Simpler shader code

**If we add rotation later:**
- Need to pass rotation matrix to shader
- Transform normals: `normalize((rotation_matrix * vec4<f32>(normal, 0.0)).xyz)`
- Slightly more expensive

### Bind Group Organization

Current setup:
- Bind Group 0: Camera uniforms (view_proj, camera_pos)

After lighting:
- Bind Group 0: Camera uniforms
- Bind Group 1: Light uniform

Update pipeline layout:
```rust
bind_group_layouts: &[&camera_bind_group_layout, &light_bind_group_layout]
```

## Implementation Phases

### Phase 1: Basic Directional Light (Recommended Start)
- Add normals to vertices
- Implement ambient + diffuse only (skip specular)
- Verify lighting works visually
- **Goal:** Get basic lighting working quickly

### Phase 2: Add Specular (Blinn-Phong Complete)
- Add camera position to uniforms
- Implement specular calculation with half-vector
- Tune shininess parameter (16-128 range, 32 is good default)
- **Goal:** Complete Blinn-Phong implementation

### Phase 3: Performance Testing
- Measure FPS with/without lighting
- Verify 1000 cubes still performs well
- Document results for future reference
- **Goal:** Validate performance assumptions

### Phase 4: Enhancement (Optional)
- Experiment with multiple directional lights
- Try per-vertex lighting for comparison
- Add light color/direction controls
- **Goal:** Explore possibilities for future features

## References

- [Learn Wgpu - Working with Lights](https://sotrh.github.io/learn-wgpu/intermediate/tutorial10-lighting/) - Complete Rust/wgpu implementation
- [WebGPU Fundamentals - Directional Lighting](https://webgpufundamentals.org/webgpu/lessons/webgpu-lighting-directional.html) - Conceptual guide
- [LearnOpenGL - Blinn-Phong](https://learnopengl.com/Advanced-Lighting/Advanced-Lighting) - Theory (OpenGL but concepts apply)
- [WebGPU Fundamentals - WGSL](https://webgpufundamentals.org/webgpu/lessons/webgpu-wgsl.html) - Shader language reference
- [Tour of WGSL](https://google.github.io/tour-of-wgsl/) - Interactive language guide

## Decision

**Implement Blinn-Phong directional lighting in phases:**
1. Start with ambient + diffuse (Phase 1)
2. Add specular (Phase 2)
3. Test performance (Phase 3)
4. Enhance if time permits (Phase 4)

**Rationale:**
- Blinn-Phong is the industry standard for real-time lighting
- Directional light is simplest and most performant
- Phased approach reduces risk and allows incremental testing
- Performance impact should be minimal (5-15% FPS reduction)
- Foundation for future lighting enhancements (point/spot lights, shadows)

**Success criteria:**
- Cubes have visible shading based on light direction
- Performance maintains 50+ FPS with 1000 cubes
- Zero compiler warnings
- Code is clean and well-documented
