# Next Steps - Immediate Action Plan

**Date:** 2026-02-17
**Status:** Phase 1 - Week 1 Complete! 🎉

## Documentation Complete ✅

- ✅ [GAME_DESIGN.md](./GAME_DESIGN.md) - Complete game vision
- ✅ [engine-requirements.md](./research/engine-requirements.md) - All engine systems mapped
- ✅ [rendering-architecture.md](./research/rendering-architecture.md) - Why wgpu
- ✅ [ecs-choice.md](./research/ecs-choice.md) - ECS research and decision
- ✅ [CLAUDE.md](../CLAUDE.md) - Development guidelines
- ✅ Testing strategy decided (balanced approach)

## Design Decisions Made ✅

1. **Art Style:** PA-style + organic aesthetic (2-3 subdivision levels)
2. **Unit Count Target:** 500-1000+ units (bevy_ecs chosen for performance)
3. **Map Size:** ~128x128 tiles (medium-sized)
4. **Abilities:** Modular system (1 attack + 1 secondary, swappable pre-game)
5. **Terrain:** TBD (will decide when implementing terrain system)
6. **Fog of War:** TBD (post-MVP)

---

## Phase 1: Engine Foundation (IN PROGRESS)

### Week 1: ECS Research & Implementation ✅ COMPLETE

**Completed Tasks:**
1. ✅ Research ECS options
   - Created `docs/research/ecs-choice.md`
   - Analyzed bevy_ecs, hecs, specs, custom
   - **Decision:** bevy_ecs (best performance, flexible, determinism achievable)

2. ✅ Integrate bevy_ecs
   - Added to `Cargo.toml`
   - Created `src/engine/` module structure
   - Implemented components (Transform, Velocity, Color, Lifetime)
   - Implemented systems (movement, bounds, lifetime)

3. ✅ Tested with 1000 entities
   - Spawned 1000 colored cubes
   - Movement system working
   - Bounds wrapping working
   - Instanced rendering: **1 draw call for all entities**
   - **Performance: 60 FPS @ 1000 entities**

**Success Criteria Met:**
- ✅ Can spawn/despawn entities
- ✅ Components work correctly
- ✅ Systems update in order
- ✅ 1000+ entities at 60 FPS
- ✅ **BONUS:** Instanced rendering optimized

---

### Week 2: Camera & Input

**Tasks:**
1. Research camera implementation
   - Create `docs/research/camera-system.md`
   - RTS camera math (perspective with low FOV)
   - Screen-to-world ray casting for mouse picking

2. Implement RTS Camera
   - Create `src/engine/camera.rs`
   - Perspective projection (configurable FOV)
   - View matrix on X/Z plane only
   - Edge scrolling + WASD movement
   - Zoom in/out
   - Map bounds clamping

3. Input System
   - Create `src/engine/input.rs`
   - Mouse state tracking (position, buttons, wheel)
   - Keyboard state tracking (keys down, just pressed)
   - Integration with winit events

4. Update rendering to use camera
   - Pass camera view+projection to shaders
   - Free camera movement

**Success Criteria:**
- Camera moves smoothly around scene
- Can zoom in/out
- Camera clamped to bounds
- Input feels responsive

---

### Week 3: Debug Tools

**Tasks:**
1. Debug Renderer
   - Create `src/engine/debug.rs`
   - Draw 3D lines (for paths, bounds)
   - Draw 3D boxes (for collision volumes)
   - Draw 3D spheres
   - Draw 2D text overlays (for labels)

2. Profiler
   - Create `src/engine/profiler.rs`
   - Track system execution times
   - Frame time tracking
   - Memory usage (entity count, component count)
   - Display as overlay

3. FPS Counter
   - Simple frame time display
   - Frame time graph (last 100 frames)

4. Console (Basic)
   - Text input overlay
   - Command parsing (spawn entity, etc.)
   - Command registration system

**Success Criteria:**
- Can see FPS and frame time
- Can profile ECS systems
- Can spawn entities via console
- Debug visualization helps development

---

### Phase 1 Deliverable

**Demo:**
- Free camera flying around 3D space
- Press key to spawn colored cubes
- FPS counter shows performance
- Console command: `spawn cube 100` creates 100 cubes
- Debug overlay shows entity count
- Smooth 60 FPS with 1000+ entities

---

## Research Documents to Create

Priority order:

1. **HIGH:** `docs/research/ecs-choice.md` (Week 1)
   - Compare bevy_ecs, hecs, specs
   - Determinism support
   - Performance characteristics
   - Make decision

2. **HIGH:** `docs/research/camera-system.md` (Week 2)
   - RTS camera math
   - Ray casting for mouse picking
   - Frustum culling (optional)

3. **MEDIUM:** `docs/research/procedural-modeling.md` (Week 4-6)
   - Skin modifier algorithm research
   - Subdivision surface algorithms (Catmull-Clark vs Loop)
   - Mesh generation pipeline
   - Caching strategy

4. **MEDIUM:** `docs/research/pathfinding.md` (Week 10-12)
   - A* vs Flowfields
   - Steering behaviors
   - Dynamic obstacles

5. **LOW:** `docs/research/ui-system.md` (Week 14+)
   - egui vs custom
   - UI architecture

6. **LOW:** `docs/research/determinism-and-replays.md` (Week 27+)
   - Fixed timestep
   - Deterministic math
   - Replay file format

---

## Immediate Next Steps (Current Sprint)

**Status:** Week 1 complete, ready for Week 2!

### Recommended: Week 2 - Camera & Input

This is the natural next step. With ECS and rendering working, we need camera controls to navigate the scene.

**Tasks:**
1. **Add depth buffer** (Quick win!)
   - Currently cubes have no depth testing (back faces visible)
   - Add depth texture and depth stencil state
   - ~1 hour task

2. **Implement RTS Camera**
   - Create `src/engine/camera.rs`
   - Perspective projection (low FOV 15-25°)
   - Planar movement (X/Z only, fixed height)
   - WASD movement
   - Mouse wheel zoom
   - Edge scrolling (optional for now)
   - Map bounds clamping

3. **Input System**
   - Create `src/engine/input.rs`
   - Mouse position tracking
   - Keyboard state tracking
   - Integration with winit events

**Success Criteria:**
- ✅ Camera moves smoothly with WASD
- ✅ Zoom in/out works
- ✅ Camera clamped to bounds
- ✅ Depth testing prevents visual artifacts

**Estimated Time:** 1-2 days

---

## Alternative Next Steps

### Option B: Debug Tools First
Before camera, add FPS counter, profiler, and debug rendering. Makes development easier going forward.

### Option C: Procedural Modeling Research
Jump ahead to the unique system. Research skin modifier and subdivision surface algorithms.

### Option D: Polish Current State
Fix compiler warnings, add tests, refactor for cleaner code, improve documentation.

---

## Recommendation

**Start Week 2: Camera & Input**

Reasons:
1. Natural progression (ECS → Rendering → Camera → Gameplay)
2. Makes testing and development much easier
3. Depth buffer is a quick win that improves visuals
4. RTS camera is core to the game feel
5. Keeps momentum going with visible progress

Let me know when you're ready to proceed! 🚀
