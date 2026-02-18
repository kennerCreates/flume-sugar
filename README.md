# Flume Sugar

A 3D Real-Time Strategy game built from scratch in Rust, featuring procedural modeling and a custom game engine.

## Vision

An RTS in the style of StarCraft 2's campaign, with:
- **Procedural aesthetics** - All models created through vertex graph → skin → subdivision pipeline
- **Modular abilities** - Units have swappable ability components (pre-game customization)
- **Custom engine** - Built on wgpu and bevy_ecs for full control
- **Deterministic simulation** - Support for replays and potential future multiplayer

## Current Status

**Phase 1 - Engine Foundation** (in progress)

**Implemented:**
- Instanced rendering pipeline (1000 entities, 1 draw call, 60 FPS)
- ECS integration (bevy_ecs) with Transform, Velocity, Color components
- Movement and bounds systems
- Blinn-Phong lighting with directional light
- Depth buffer for proper 3D occlusion
- In-game debug overlay (egui) — toggle with F3
  - FPS, frame time (avg/min/max), entity count, draw calls, resolution, camera info

**Next up:**
- RTS camera system (WASD, edge scrolling, zoom)
- Input system (mouse + keyboard state tracking)
- Procedural modeling system (skin + subdivision)

## Build & Run

```bash
cargo run           # debug build
cargo run --release # release build (much faster)
```

**Controls:**
- **F3** — Toggle debug overlay
- **Escape** — Quit

**Requirements:**
- Rust 1.93+ (edition 2024)
- GPU with Vulkan, DirectX 12, or Metal support

## Project Structure

```
flume_sugar/
├── docs/
│   ├── GAME_DESIGN.md         # Game vision and design
│   ├── DESIGN_DECISIONS.md    # Locked-in technical decisions
│   ├── NEXT_STEPS.md          # Immediate action plan
│   └── research/              # Technical research documents
│       ├── ecs-choice.md
│       ├── rendering-architecture.md
│       ├── engine-requirements.md
│       └── lighting-implementation.md
├── src/
│   ├── engine/                # Reusable engine components
│   │   ├── components.rs      # ECS components (Transform, Velocity, Color)
│   │   ├── debug_overlay.rs   # In-game debug UI (egui)
│   │   ├── systems.rs         # ECS systems (placeholder)
│   │   └── mod.rs
│   ├── main.rs                # Application entry point
│   └── shader_instanced.wgsl  # GPU shaders with instancing + lighting
└── Cargo.toml
```

## Technology Stack

- **Graphics:** wgpu 23.0 (cross-platform, modern API)
- **ECS:** bevy_ecs 0.15 (high-performance entity management)
- **UI:** egui 0.30 (immediate-mode GUI, debug overlay + future HUD)
- **Math:** glam 0.29 (vectors, matrices, quaternions)
- **Windowing:** winit 0.30
- **Language:** Rust (edition 2024)

## Design Philosophy

- **Engine vs Game separation** — `src/engine/` is reusable and game-agnostic; game-specific logic will live in `src/game/`
- **Research-driven** — Major decisions documented in `docs/research/` to preserve context
- **Balanced testing** — Unit tests for algorithms, manual testing for graphics, benchmarks for critical paths

## Performance Targets

- **500-1000 units** on screen at 60 FPS
- **128x128 tile maps** (medium-sized)
- **2-3 subdivision levels** for procedural models
- Deterministic simulation for replays

## Documentation

- [GAME_DESIGN.md](docs/GAME_DESIGN.md) - Full game design document
- [NEXT_STEPS.md](docs/NEXT_STEPS.md) - Current roadmap and priorities
- [engine-requirements.md](docs/research/engine-requirements.md) - All engine systems planned
