# Flume Sugar

A 3D Real-Time Strategy game built from scratch in Rust, featuring procedural modeling and a custom game engine.

## Vision

An RTS in the style of StarCraft 2's campaign, with:
- **Procedural aesthetics** - All models created through vertex graph → skin → subdivision pipeline
- **Modular abilities** - Units have swappable ability components (pre-game customization)
- **Custom engine** - Built on wgpu and bevy_ecs for full control
- **Deterministic simulation** - Support for replays and potential future multiplayer

## Current Status

🚧 **Early Development** - Engine foundation in progress

**Implemented:**
- ✅ Basic wgpu rendering pipeline
- ✅ ECS integration (bevy_ecs)
- ✅ Entity spawning and management
- ✅ Basic movement and bounds systems

**Next up:**
- ⬜ Fix per-entity transforms in rendering
- ⬜ Instanced rendering for performance
- ⬜ RTS-style camera system
- ⬜ Procedural modeling system (skin + subdivision)

## Build & Run

```bash
# Debug build
cargo build
cargo run

# Release build (much faster!)
cargo build --release
cargo run --release
```

**Requirements:**
- Rust 1.93+ (edition 2024)
- GPU with Vulkan, DirectX 12, or Metal support

## Project Structure

```
flume_sugar/
├── docs/
│   ├── CLAUDE.md              # Development guidelines
│   ├── GAME_DESIGN.md         # Game vision and design
│   ├── DESIGN_DECISIONS.md    # Locked-in technical decisions
│   ├── NEXT_STEPS.md          # Immediate action plan
│   └── research/              # Technical research documents
│       ├── rendering-architecture.md
│       ├── ecs-choice.md
│       └── engine-requirements.md
├── src/
│   ├── engine/                # Reusable engine components
│   │   ├── components.rs      # ECS components (Transform, Velocity, etc.)
│   │   ├── systems.rs         # ECS systems (movement, etc.)
│   │   └── mod.rs
│   ├── main.rs                # Application entry point
│   └── shader.wgsl            # GPU shaders (WGSL)
└── Cargo.toml
```

## Design Philosophy

**Engine vs Game Separation:**
- `src/engine/` - Reusable, game-agnostic systems
- `src/game/` - (future) Game-specific logic and content
- Goal: Reuse engine for future games without modification

**Research-Driven Development:**
- Major decisions documented in `docs/research/`
- Avoid re-researching solved problems
- Preserve context and rationale

**Balanced Testing:**
- Unit tests for algorithms (pathfinding, collision, determinism)
- Manual testing for graphics and gameplay
- Performance benchmarks for critical systems

## Technology Stack

- **Graphics:** wgpu 23.0 (cross-platform, modern API)
- **ECS:** bevy_ecs 0.15 (high-performance entity management)
- **Math:** glam 0.29 (vectors, matrices, quaternions)
- **Windowing:** winit 0.30
- **Language:** Rust (edition 2024)

## Documentation

See `docs/` for detailed documentation:
- [CLAUDE.md](docs/CLAUDE.md) - How to work with this codebase
- [GAME_DESIGN.md](docs/GAME_DESIGN.md) - Full game design document
- [engine-requirements.md](docs/research/engine-requirements.md) - All engine systems planned

## Performance Targets

- **500-1000 units** on screen at 60 FPS
- **128x128 tile maps** (medium-sized)
- **2-3 subdivision levels** for procedural models
- Deterministic simulation for replays

## License

TBD (Not yet licensed)

## Development

This is a learning project and personal game engine. Development is iterative with heavy documentation of decisions and research.

**Current Phase:** Phase 1 - Engine Foundation (ECS, rendering, camera)

---

Built with 🦀 Rust and ❤️
