# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## ⚠️ Maintaining This Document

**CRITICAL:** After implementing any feature or making significant changes, you MUST:
1. Review this CLAUDE.md file for accuracy
2. Update any outdated sections (architecture, file structure, patterns)
3. Document new systems or major decisions
4. Update the research documentation (see Research Documentation section)

This ensures future Claude instances don't waste time rediscovering context or re-researching solved problems.

## Project Overview

Flume Sugar is a 3D RTS game built with Rust and wgpu. Code is organized into modules under `src/` by system (camera, input, rendering, etc.). If code proves reusable in a future project, it can be extracted then — don't design for reuse upfront.

## Build & Run

```bash
# Build the project
cargo build

# Run the application
cargo run

# Build optimized release version
cargo build --release
cargo run --release
```

**Controls:**
- MMB drag — pan camera (grab-the-world)
- MMB + RMB drag simultaneously — rotate camera yaw (drag left/right)
- Mouse wheel — zoom in/out
- Mouse edge scroll — pan camera when MMB is not held
- F3 — toggle debug overlay
- ESC or close window to exit
- Window resizing is supported

## Code Organization Philosophy

Code is organized by system under `src/`. Use `src/engine/` for lower-level systems (rendering, input, camera) and `main.rs` for higher-level game logic. Move code when the current location becomes inconvenient — not in advance. If code proves useful in a future project, extract it then.

### Research Documentation

**Location:** `docs/research/` directory

**When to write one:** Only when making a genuine architectural choice between meaningfully different approaches — where the decision isn't obvious and future-you would benefit from knowing why this option was chosen over others. If there's essentially one sensible approach, skip the research doc.

**Good candidates:** ECS library selection, rendering API choice, UI framework, pathfinding algorithm, networking model.
**Skip for:** Implementing a well-understood system (RTS camera, input state), adding a feature with no real alternatives.

**Format when you do write one:**
- **Problem statement**: What are we trying to solve?
- **Options considered**: List alternatives with pros/cons
- **Decision**: What approach was chosen and why
- **Implementation notes**: Key insights, gotchas, performance considerations
- **References**: Links to articles, documentation, examples used

**Purpose:** Avoid re-researching the same topics. When revisiting a system, read the research doc first to understand the context and rationale for current implementation.

## Architecture

### Module Structure

```
src/
  main.rs              — App entry point, State struct, winit event loop, build_procedural_sphere()
  engine/
    mod.rs             — Module registration and re-exports
    camera.rs          — RtsCamera: RTS-style camera with WASD/zoom/edge-scroll
    components.rs      — ECS components (Transform, Color)
    debug_overlay.rs   — egui debug overlay (F3)
    input.rs           — InputState: keyboard and mouse state from winit events
    mesh.rs            — GpuVertex, PolyMesh, RenderMesh, triangulate_smooth()
    skin.rs            — SkinGraph, skin_modifier() (vertex graph → quad PolyMesh)
    subdivide.rs       — catmull_clark(), subdivide() (Catmull-Clark subdivision)
    systems.rs         — ECS systems (placeholder)
```

### Camera System (`src/engine/camera.rs`)

`RtsCamera` implements a top-down RTS camera:
- **Target**: A `Vec2` point on the XZ ground plane the camera orbits around
- **Pitch/Yaw**: Fixed elevation (55°) and adjustable horizontal rotation
- **Distance**: Adjustable zoom between `min_distance` and `max_distance`
- **FOV**: Low field of view (20°) for isometric-style perspective
- **MMB drag**: Pans target on XZ with grab-the-world feel (pan_scale formula accounts for zoom/pitch/FOV)
- **MMB + RMB drag**: Rotates yaw; drag left/right maps to radians via `rotate_sensitivity`
- **Zoom**: Mouse wheel adjusts distance
- **Edge scrolling**: Mouse near screen edges pans the target (disabled during MMB drag)
- **Bounds**: Target is clamped to configurable XZ map bounds

Key methods:
- `update(input, dt)` — call once per frame in `State::update()`
- `view_projection(aspect)` — combined VP matrix for GPU uniform
- `camera_position()` — eye position for lighting uniform

### Input System (`src/engine/input.rs`)

`InputState` centralizes winit event processing:
- `process_event(event)` — call for every `WindowEvent` before game logic
- `end_frame()` — call after update+render to reset per-frame accumulators
- `is_key_held(KeyCode)` — query keyboard state
- `scroll_delta` — accumulated vertical scroll this frame
- `mouse_position`, `mouse_delta` — cursor position and movement
- `window_size` — used for edge scroll boundary detection

### Graphics Pipeline Flow

The application follows a standard wgpu rendering pipeline:

1. **Initialization** (`State::new()`)
   - Creates wgpu instance, adapter, device, and queue
   - Configures surface for window rendering
   - Loads and compiles WGSL shaders from `src/shader_instanced.wgsl`
   - Creates render pipeline with vertex/fragment shader stages
   - Initializes vertex and index buffers with cube geometry
   - Sets up uniform buffer for transformation matrices

2. **Update Loop** (`State::update()`)
   - Updates `RtsCamera` via `camera.update(&input, dt)` (WASD, zoom, edge scroll)
   - Runs ECS movement and bounds systems
   - Camera uniform upload happens at the start of `render()` (reads final camera state)

3. **Render Loop** (`State::render()`)
   - Acquires surface texture
   - Creates command encoder
   - Begins render pass with color attachment
   - Binds pipeline, bind groups, and buffers
   - Issues indexed draw call
   - Submits commands and presents frame

### Key Components

**Geometry** (procedural pipeline — see `docs/research/procedural-modeling.md`):
- `GpuVertex { position: [f32;3], normal: [f32;3] }` — canonical vertex type (locations 0, 1)
- `PolyMesh` — intermediate n-gon mesh used during procedural generation
- `RenderMesh` — GPU-ready triangulated mesh with smooth normals and `u32` indices
- Pipeline: `SkinGraph → skin_modifier() → catmull_clark()×N → triangulate_smooth() → GPU`
- Test scene: single vertex → skin (cube) → CC×2 → near-sphere (98 verts, 576 indices)
- Index buffer uses `wgpu::IndexFormat::Uint32` (supports meshes beyond 65535 vertices)

**Transformation Pipeline** (`RtsCamera`):
- View matrix: `look_at_rh(eye, target, Y)` — eye computed from pitch/yaw/distance offset
- Projection matrix: perspective with 20° FOV (RTS isometric feel), aspect-ratio aware
- Combined as: `projection * view` uploaded as `Uniforms.view_proj`
- Per-instance model transform is the entity's `Transform.position` in the instance buffer

**Shaders** (`src/shader_instanced.wgsl`):
- Vertex shader (`vs_main`): applies view-projection matrix, passes normal for lighting
- Fragment shader (`fs_main`): Blinn-Phong shading using instance color
- Uses WGSL (WebGPU Shading Language) syntax

**Event Handling** (ApplicationHandler pattern):
- Implements `ApplicationHandler` trait for event handling
- `resumed()` - creates window and initializes state
- `window_event()` - handles window events (close, resize, keyboard, redraw)
- `about_to_wait()` - requests redraw for continuous rendering
- Uses modern winit 0.30 API with `EventLoop::run_app()`

## Code Modification Patterns

**When adding new features, always ask:**
1. Where does this naturally live? (`src/engine/` for low-level systems, `main.rs` for game logic)
2. Is this a genuine architectural choice? If yes, consider a research doc in `docs/research/`
3. After implementation, does CLAUDE.md need updating?

**To change the procedural mesh:**
- Edit `build_procedural_sphere()` in `main.rs` — change `add_node()` radius, or `subdivide()` level
- To add edges: call `graph.add_edge(a, b)` after `add_node()` (edge tube generation is stubbed; implement in `skin.rs`)

**To adjust camera behavior:**
- Edit `RtsCamera::new()` defaults in `src/engine/camera.rs` (speed, FOV, pitch, bounds)
- `move_speed` / `edge_scroll_speed` control pan speed in world units/sec
- `zoom_speed` controls how many distance units per scroll line
- `pitch` controls elevation angle (55° = good RTS view, 90° = straight down)

**To add more complex geometry:**
- Extend `Vertex` struct with additional attributes (normals, UVs)
- Update `Vertex::desc()` with new vertex attributes
- Modify shaders to handle new attributes

## Dependencies

- **wgpu 23.0**: Graphics API abstraction
- **winit 0.30**: Window creation and event handling
- **glam 0.29**: Vector and matrix math
- **bytemuck 1.18**: Safe type casting for GPU buffers
- **pollster 0.4**: Blocks on async wgpu initialization
- **env_logger 0.11**: Optional logging for wgpu internals

## Code Quality Standards

### Warning-Free Builds

**CRITICAL:** Before completing any task, ensure the build has zero warnings:

```bash
cargo build
```

**All compilation warnings must be resolved by:**
1. **Removing the unused code** - Delete imports, functions, fields, or methods that aren't used
2. **Using the code** - If it's genuinely needed now, wire it up

**Note on binary crates:** This project compiles as a binary, not a library. Rust warns about unused `pub` methods in binary crates just as it does for private ones — `pub` only exempts from dead_code warnings in library crates (where external consumers might call it). So "keep it because it's pub" doesn't work here. Either use the code or delete it.

**Do NOT:**
- Leave warnings unaddressed
- Suppress warnings with `#[allow(...)]` attributes
- Complete a task with a build that shows warnings

**Keep the codebase clean:**
- Delete unused code now; add it back when actually needed
- If you plan to add features later, note them in NEXT_STEPS.md or a comment

## Development Workflow

### Implementing a New System

1. **Research Phase** (only for genuine architectural choices)
   - If there's a real fork in the road (library selection, algorithmic approach), investigate and create `docs/research/[system-name].md`
   - If the approach is obvious, skip the research doc and go straight to implementation

2. **Implementation Phase**
   - Write code, add comments explaining non-obvious decisions
   - Reference research doc in code comments if one exists

3. **Documentation Phase**
   - Update CLAUDE.md with new architecture details

4. **Validation**
   - **Verify warning-free build** (`cargo build` shows 0 warnings)
   - Test the implementation
   - Ensure CLAUDE.md is accurate and complete

5. **Commit to Git**
   - Stage relevant files (`git add`)
   - Create descriptive commit message following project style
   - Include Co-Authored-By line for AI assistance
   - Push to GitHub (`git push origin main`)
   - **CRITICAL:** Commit after each feature implementation, not at end of session

### When Modifying Existing Systems

1. Read relevant research doc first (if exists)
2. Understand the original rationale and constraints
3. If changing approach significantly, update research doc with new findings
4. Update CLAUDE.md if architecture has changed

## Known Issues

None currently. The codebase builds with zero warnings.
