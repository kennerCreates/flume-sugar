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
   - Increments rotation angle
   - Computes transformation matrix (Model × View × Projection)
   - Uploads updated uniforms to GPU via `queue.write_buffer()`

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

**Transformation Pipeline** (`Uniforms::update_transform()`):
- Model matrix: rotates cube on Y and X axes
- View matrix: translates camera -3 units on Z axis
- Projection matrix: perspective with 45° FOV, aspect ratio 800/600
- Combined as: `projection * view * model`

**Shaders** (`src/shader.wgsl`):
- Vertex shader (`vs_main`): applies transformation matrix, passes through color
- Fragment shader (`fs_main`): outputs interpolated vertex colors
- Uses WGSL (WebGPU Shading Language) syntax

**Event Handling** (main event loop):
- Window events (close, resize, keyboard)
- Redraw requests trigger update + render cycle
- Uses deprecated `EventLoop::run()` API (winit 0.30)

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

**To move camera:**
- Edit `Vec3::new(0.0, 0.0, -3.0)` in view matrix calculation
- Adjust FOV or aspect ratio in perspective matrix

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
   - Test the implementation
   - Verify engine/game separation is maintained
   - Ensure documentation is accurate and complete

### When Modifying Existing Systems

1. Read relevant research doc first (if exists)
2. Understand the original rationale and constraints
3. If changing approach significantly, update research doc with new findings
4. Update CLAUDE.md if architecture has changed

## Known Issues

- Uses deprecated winit APIs (`EventLoop::run()`, `create_window()`)
  - Warnings don't affect functionality
  - Future: migrate to `EventLoop::run_app()` and `ActiveEventLoop::create_window()`
- No depth buffer (depth_stencil: None)
  - Back-face culling prevents visible Z-fighting
  - Add depth buffer for more complex scenes with overlapping geometry
