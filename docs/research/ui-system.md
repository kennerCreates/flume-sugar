# UI System Research & Decision

**Date:** 2026-02-18
**Status:** Decision made — egui 0.30

## Requirements

- In-game debug overlay (immediate need)
- Future player HUD: resource bars, unit selection, build menus, minimap frame
- Must render on top of existing wgpu 23 render pass
- Must integrate with winit 0.30 event loop
- Semi-transparent panels, styled text, interactive elements

## Options Evaluated

### 1. glyphon (text-only)

- **What:** Lightweight wgpu text renderer built on cosmic-text
- **Version:** 0.7.0 targets wgpu 23
- **Pros:** Minimal dependencies (~5 crates), simple API, renders directly into a render pass
- **Cons:** Text only — no panels, buttons, sliders, layout, or input handling. Every UI element beyond text would need to be built from scratch (custom quad pipeline for backgrounds, manual hit testing, layout logic)
- **Verdict:** Suitable for a debug overlay alone, but doesn't scale to a full HUD

### 2. egui (immediate-mode GUI)

- **What:** Full immediate-mode GUI library for Rust
- **Version:** 0.30.0 depends on wgpu 23.0.0 and winit 0.30 (exact match)
- **Crates:** `egui`, `egui-wgpu`, `egui-winit`
- **Pros:**
  - Complete widget set: labels, buttons, sliders, panels, windows, drag-and-drop, images
  - Renders into an existing wgpu render pass via `egui_wgpu::Renderer` — no separate window or framework takeover
  - `egui_winit::State` handles input translation from winit events automatically
  - Immediate-mode API is simple: build UI every frame, no retained widget tree or callback wiring
  - Highly customizable visuals (colors, fonts, margins, transparency)
  - ~50 lines of integration glue, no eframe dependency needed
  - Large ecosystem and active maintenance
- **Cons:**
  - Larger dependency tree than glyphon
  - Immediate-mode redraws every frame (minimal cost when hidden)
  - Not designed for heavy animation or complex game-style UI (but sufficient for RTS HUD)
- **Verdict:** Best fit — handles both debug overlay and future HUD

### 3. iced (retained-mode, Elm-style)

- **What:** Cross-platform GUI library with Elm-inspired architecture
- **Version:** Uses wgpu for rendering on desktop
- **Pros:** Strong for complex multi-screen applications, type-safe message passing
- **Cons:** Retained-mode architecture is more boilerplate for a game overlay. Designed more for standalone applications than in-engine overlays. Harder to embed into an existing render loop.
- **Verdict:** Overkill for game HUD, better suited for standalone tools/editors

### 4. Custom bitmap font renderer

- **What:** Embed a monospace bitmap font atlas, render textured quads manually
- **Pros:** Zero external dependencies, full control
- **Cons:** Significant implementation effort (texture loading, glyph layout, quad pipeline, alpha blending, hit testing). Essentially rebuilding what egui already provides, but worse. Not aligned with project velocity goals.
- **Verdict:** Too much work for the benefit

## Decision: egui 0.30

egui provides the best balance of simplicity, capability, and compatibility:

- Version-locked to our exact wgpu 23 + winit 0.30 stack
- Immediate-mode API fits naturally into a game loop
- Scales from a simple debug overlay to a full player HUD without switching systems
- No eframe needed — `egui-wgpu` and `egui-winit` integrate directly

## Integration Architecture

```
winit events → egui_winit::State::on_window_event()
             → egui::Context::run(raw_input, |ctx| { build UI })
             → tessellate → ClippedPrimitives
             → egui_wgpu::Renderer renders into a second wgpu render pass
               (LoadOp::Load to composite over 3D scene, no depth attachment)
```

Key integration points in the codebase:
- `src/engine/debug_overlay.rs` — DebugOverlay struct wrapping egui state
- `src/main.rs` — event forwarding, F3 toggle, render pass after 3D scene

## Dependencies Added

```toml
egui = "0.30"
egui-wgpu = "0.30"
egui-winit = "0.30"
```

## References

- egui repository: https://github.com/emilk/egui
- egui-wgpu 0.30 API docs: https://docs.rs/egui-wgpu/0.30.0/egui_wgpu/
- egui without eframe example: https://github.com/matthewjberger/wgpu-example
- egui 0.30 workspace Cargo.toml (confirming wgpu 23): https://github.com/emilk/egui/blob/0.30.0/Cargo.toml
