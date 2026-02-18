# Next Steps - Immediate Action Plan

**Last Updated:** 2026-02-18
**Status:** Phase 1 - Weeks 1-3 Complete

## Completed

### Week 1: ECS & Rendering Foundation
- bevy_ecs integration with Transform, Velocity, Color components
- Instanced rendering pipeline (1000 entities, 1 draw call, 60 FPS)
- Blinn-Phong lighting with directional light
- Depth buffer for proper 3D occlusion
- Movement and bounds systems

### Week 2: Debug Overlay (egui)
- Integrated egui (0.30) as the UI system (egui-wgpu + egui-winit)
- In-game debug overlay toggled with F3
- Displays: FPS, frame time (avg/min/max), entity count, draw calls, resolution, camera info
- Styled with semi-transparent black panel, small white monospace font
- egui will also serve as the foundation for the future player HUD

### Week 3: Camera & Input
- `src/engine/input.rs` — `InputState` for keyboard/mouse/scroll tracking from winit events
- `src/engine/camera.rs` — `RtsCamera` with WASD pan, mouse wheel zoom, edge scrolling, bounds clamping
- 20° FOV perspective projection (RTS isometric feel), 55° fixed pitch
- Camera moves relative to its facing direction (yaw-aware WASD)
- Refactored camera uniforms out of inline render() into `RtsCamera` methods
- Debug overlay updated to show camera target position, distance, and zoom %

---

## Future Priorities

| Priority | System | Notes |
|----------|--------|-------|
| HIGH | Procedural modeling (skin + subdivision) | Core visual identity — biggest technical risk |
| HIGH | Pathfinding (A* or flowfield) | Essential for RTS unit movement |
| MEDIUM | Terrain/map rendering | Ground plane, 128x128 tile grid |
| MEDIUM | Selection & commands | Click-to-select, right-click-to-move |
| MEDIUM | Combat & resource systems | Core RTS gameplay loop |
| MEDIUM | Animation system | Procedural + baked animations |
| LOW | Audio | Sound effects and music |
| LOW | Save/load & replays | Deterministic simulation |

## Research Documents

| Status | Document | Topic |
|--------|----------|-------|
| Done | [ecs-choice.md](./research/ecs-choice.md) | ECS comparison and decision |
| Done | [rendering-architecture.md](./research/rendering-architecture.md) | Why wgpu |
| Done | [engine-requirements.md](./research/engine-requirements.md) | All engine systems mapped |
| Done | [lighting-implementation.md](./research/lighting-implementation.md) | Blinn-Phong lighting |
| Done | [ui-system.md](./research/ui-system.md) | UI approach — egui chosen |
| Done | camera-system.md | RTS camera math, ray casting |
| TODO | procedural-modeling.md | Skin modifier, subdivision surfaces |
| TODO | pathfinding.md | A* vs flowfields, steering |
