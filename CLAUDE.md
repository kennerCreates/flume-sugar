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

Flume Sugar is a 3D game project built with Rust and wgpu. The project is structured to separate reusable engine components from game-specific code, allowing the engine to be reused for future games without modification.

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
- WASD — pan camera
- Mouse wheel — zoom in/out
- Mouse edge scroll — pan camera (move mouse to screen edge)
- F3 — toggle debug overlay
- ESC or close window to exit
- Window resizing is supported

## Code Organization Philosophy

### Engine vs Game Separation

**Engine code** (reusable, game-agnostic):
- Rendering system (wgpu pipeline, shaders, vertex/index management)
- ECS (Entity Component System) - when implemented
- Input handling abstractions
- Resource management (textures, models, audio)
- Physics integration
- Scene graph / transform hierarchy
- Camera systems
- Core utilities and math

**Game code** (specific to current game):
- Game logic and rules
- Specific entity behaviors and components
- Level data and progression
- UI layouts and menus specific to this game
- Game-specific assets and configuration

**Implementation strategy:**
- Currently all code is in `src/main.rs` (bootstrap phase)
- As systems grow, split into modules: `src/engine/` and `src/game/`
- Consider separate crates (`flume_engine` and `flume_game`) if reusability becomes important
- Engine code should never depend on game code
- Game code can freely use engine APIs

### Research Documentation

**Location:** `docs/research/` directory

**Format:** Create a markdown file for each major system or technical decision.

**Required contents:**
- **Problem statement**: What are we trying to solve?
- **Options considered**: List alternatives with pros/cons
- **Decision**: What approach was chosen and why
- **Implementation notes**: Key insights, gotchas, performance considerations
- **References**: Links to articles, documentation, examples used

**Example topics:**
- `docs/research/rendering-architecture.md` - wgpu pipeline design decisions
- `docs/research/ecs-choice.md` - Which ECS library to use (bevy_ecs, hecs, specs, custom)
- `docs/research/asset-pipeline.md` - How to load and manage assets
- `docs/research/physics-integration.md` - Which physics engine and integration approach

**Purpose:** Avoid re-researching the same topics. When revisiting a system, read the research doc first to understand the context and rationale for current implementation.

## Architecture

### Module Structure

```
src/
  main.rs              — App entry point, State struct, winit event loop
  engine/
    mod.rs             — Module registration and re-exports
    camera.rs          — RtsCamera: RTS-style camera with WASD/zoom/edge-scroll
    components.rs      — ECS components (Transform, Velocity, Color)
    debug_overlay.rs   — egui debug overlay (F3)
    input.rs           — InputState: keyboard and mouse state from winit events
    systems.rs         — ECS systems (placeholder)
```

### Camera System (`src/engine/camera.rs`)

`RtsCamera` implements a top-down RTS camera:
- **Target**: A `Vec2` point on the XZ ground plane the camera orbits around
- **Pitch/Yaw**: Fixed elevation (55°) and horizontal rotation (0 = facing -Z)
- **Distance**: Adjustable zoom between `min_distance` and `max_distance`
- **FOV**: Low field of view (20°) for isometric-style perspective
- **Movement**: WASD pans the target on XZ relative to camera facing direction
- **Zoom**: Mouse wheel adjusts distance
- **Edge scrolling**: Mouse near screen edges pans the target
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
   - Loads and compiles WGSL shaders from `src/shader.wgsl`
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

**Geometry** (`VERTICES` and `INDICES` constants):
- 8 vertices defining cube corners with position and color attributes
- 36 indices (6 faces × 2 triangles × 3 vertices) defining triangle topology
- Counter-clockwise winding order for front faces

**Transformation Pipeline** (`RtsCamera`):
- View matrix: `look_at_rh(eye, target, Y)` — eye computed from pitch/yaw/distance offset
- Projection matrix: perspective with 20° FOV (RTS isometric feel), aspect-ratio aware
- Combined as: `projection * view` uploaded as `Uniforms.view_proj`
- Per-instance model transform is the entity's `Transform.position` in the instance buffer

**Shaders** (`src/shader.wgsl`):
- Vertex shader (`vs_main`): applies transformation matrix, passes through color
- Fragment shader (`fs_main`): outputs interpolated vertex colors
- Uses WGSL (WebGPU Shading Language) syntax

**Event Handling** (ApplicationHandler pattern):
- Implements `ApplicationHandler` trait for event handling
- `resumed()` - creates window and initializes state
- `window_event()` - handles window events (close, resize, keyboard, redraw)
- `about_to_wait()` - requests redraw for continuous rendering
- Uses modern winit 0.30 API with `EventLoop::run_app()`

## Code Modification Patterns

**When adding new features, always ask:**
1. Is this engine code or game code?
2. If engine code, could it be reused in a different game?
3. Does this require research? If yes, document findings in `docs/research/`
4. After implementation, does CLAUDE.md need updating?

**To change cube appearance:**
- Modify `VERTICES` array for different colors or size
- Edit `INDICES` for different topology

**To adjust animation:**
- Change rotation increment in `State::update()` (currently `0.01`)
- Modify rotation axes in `Mat4::from_rotation_*()` calls

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
1. **Removing unused code** - Delete imports, functions, structs, fields, or methods that aren't used
2. **Using the code** - If it's meant to be used, implement its usage now

**Do NOT:**
- Leave warnings unaddressed
- Suppress warnings with `#[allow(...)]` attributes
- Complete a task with a build that shows warnings
- Keep "future use" code that isn't currently needed

**Keep the codebase clean:**
- If you plan to add features later, document plans in todo lists or research docs
- Add the code when you actually need it, not before
- Trust that you can always add code back later when needed

**Why this matters:**
- Warnings indicate potential bugs, dead code, or API deprecations
- Warning-free builds maintain code quality and prevent warning fatigue
- Future changes are easier when the codebase starts clean

## Development Workflow

### Implementing a New System

1. **Research Phase**
   - Investigate options and approaches
   - Create research doc in `docs/research/[system-name].md`
   - Document problem, options, decision, and rationale

2. **Implementation Phase**
   - Determine if engine or game code
   - Write code following engine/game separation
   - Add comments explaining non-obvious decisions
   - Reference research doc in code comments if applicable

3. **Documentation Phase**
   - Update CLAUDE.md with new architecture details
   - Update research doc with implementation insights
   - Document any deviations from planned approach

4. **Validation**
   - **Verify warning-free build** (`cargo build` shows 0 warnings)
   - Test the implementation
   - Verify engine/game separation is maintained
   - Ensure documentation is accurate and complete

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
