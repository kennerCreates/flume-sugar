# Rendering Architecture Research

**Date:** 2026-02-17
**Status:** Decided - wgpu selected
**Last Updated:** 2026-02-17

## Problem Statement

Need to choose a graphics API/framework for rendering 3D graphics in a Rust game. The solution should:
- Be modern and maintainable
- Support multiple platforms (Windows, macOS, Linux)
- Have good Rust integration
- Be suitable for learning graphics programming fundamentals
- Provide reasonable performance for a 3D game

## Options Considered

### 1. wgpu (WebGPU)
**Pros:**
- Modern, safe Rust API wrapping Vulkan/Metal/DirectX 12
- Cross-platform (Windows, Mac, Linux, WebAssembly)
- Future-proof - follows WebGPU standard
- Strong safety guarantees prevent many common graphics bugs
- Active development and maintenance
- Good documentation and examples

**Cons:**
- Slightly steeper learning curve than older APIs
- More boilerplate for simple scenes
- Fewer learning resources compared to OpenGL

### 2. OpenGL (via glium/glow)
**Pros:**
- Mature, well-documented API
- Extensive learning resources available
- Simpler initial setup
- Familiar to many developers

**Cons:**
- Legacy/deprecated technology
- Less performant than modern APIs
- State-machine design leads to bugs
- No guaranteed future support
- Not as "safe" in Rust context

### 3. Bevy Game Engine
**Pros:**
- Complete game engine with rendering, ECS, physics
- Very easy to get started (20 lines for a cube)
- Modern architecture
- Strong Rust ecosystem integration

**Cons:**
- Not learning graphics fundamentals - abstracts too much
- Goal is to learn WITHOUT a game engine
- Harder to separate engine from game code
- Less control over rendering pipeline

## Decision

**Selected: wgpu**

**Rationale:**
1. **Learning Goals**: Provides the right level of abstraction - low enough to understand graphics programming, high enough to be productive
2. **Modern Best Practices**: Learn current techniques, not legacy approaches
3. **Future-Proof**: Code written today will remain relevant
4. **Rust Integration**: Takes advantage of Rust's safety guarantees
5. **Separation Goals**: Lower-level than Bevy, easier to separate engine from game code
6. **Platform Support**: Can eventually target web via WebAssembly if desired

## Implementation Notes

### Architecture Decisions

**Pipeline Structure:**
- Using indexed rendering with vertex/index buffers
- Separate vertex and fragment shader stages
- Uniform buffers for transformation matrices
- WGSL (WebGPU Shading Language) for shaders

**Vertex Format:**
- Position: `[f32; 3]` - 3D coordinates
- Color: `[f32; 3]` - RGB values
- Future: Add normals `[f32; 3]` for lighting
- Future: Add UVs `[f32; 2]` for textures

**Transformation Pipeline:**
- Standard Model-View-Projection (MVP) matrix approach
- Model matrix: object transformations (rotation, scale, position)
- View matrix: camera position and orientation
- Projection matrix: perspective projection (45° FOV)
- Combined as: `projection * view * model`

### Key Dependencies

- **wgpu 23.0**: Core graphics API
- **winit 0.30**: Window management (pairs naturally with wgpu)
- **glam 0.29**: Math library for vectors and matrices (fast, well-tested)
- **bytemuck 1.18**: Safe type casting for GPU buffers (zero-cost abstractions)
- **pollster 0.4**: Simple async executor for wgpu initialization

### Performance Considerations

- Using indexed rendering reduces vertex duplication (8 vertices instead of 36)
- Uniform buffers updated once per frame, not per vertex
- Back-face culling enabled to skip invisible faces
- No depth buffer yet - will add when rendering overlapping geometry

### Gotchas Discovered

1. **Async Initialization**: wgpu uses async APIs for device creation
   - Solution: Use pollster to block on async in main thread

2. **Vertex Layout**: GPU expects specific memory layout
   - Solution: `#[repr(C)]` ensures C-compatible layout
   - bytemuck ensures safe casting to bytes

3. **Coordinate Systems**: Different from OpenGL
   - wgpu uses Y-down for screen space
   - Right-handed coordinate system for world space

4. **Shader Entry Points**: In wgpu 23.0, entry points are now `Option<&str>`
   - Must wrap in `Some("vs_main")` instead of bare `"vs_main"`

5. **winit API Changes**: Version 0.30 changed window creation
   - Use `Window::default_attributes()` instead of `WindowBuilder`
   - Some APIs deprecated but still functional

## Future Considerations

### When to Add Depth Buffer
Add depth testing when:
- Rendering overlapping objects
- Implementing 3D scenes with complex geometry
- Z-fighting becomes visible

Current back-face culling is sufficient for single rotating cube.

### Texture Support
Will need to add:
- Texture coordinate attributes to vertex format
- Sampler and texture bind groups
- Texture loading utilities (consider `image` crate)

### Lighting System
Options to research:
- Forward rendering with multiple lights
- Deferred shading for many lights
- PBR (Physically Based Rendering) materials
- See `docs/research/lighting-system.md` when implemented

### Compute Shaders
wgpu supports compute shaders for:
- Particle systems
- Physics simulations
- Procedural generation
- GPU-accelerated algorithms

## References

- [wgpu documentation](https://docs.rs/wgpu/)
- [Learn wgpu tutorial](https://sotrh.github.io/learn-wgpu/)
- [WebGPU specification](https://www.w3.org/TR/webgpu/)
- [glam math library](https://docs.rs/glam/)
- [wgpu examples](https://github.com/gfx-rs/wgpu/tree/trunk/examples)

## Conclusions

wgpu was the right choice for this project. The initial setup is more verbose than OpenGL, but:
- The safety guarantees catch bugs at compile time
- The modern API design feels natural in Rust
- Cross-platform support is excellent
- Performance is excellent for our needs

The separation between engine and game code will be easier with wgpu's explicit resource management compared to OpenGL's global state machine.
