# Next Steps - Immediate Action Plan

**Date:** 2026-02-17
**Status:** Ready to begin Phase 1

## Documentation Complete ✅

- ✅ [GAME_DESIGN.md](./GAME_DESIGN.md) - Complete game vision
- ✅ [engine-requirements.md](./research/engine-requirements.md) - All engine systems mapped
- ✅ [rendering-architecture.md](./research/rendering-architecture.md) - Why wgpu
- ✅ [CLAUDE.md](../CLAUDE.md) - Development guidelines
- ✅ Testing strategy decided (balanced approach)

## Open Design Questions

Before diving deep into content creation, we should answer these:

1. **Art Style:** More angular/geometric or smooth/organic after subdivision?
2. **Unit Count Target:** 100? 500? 1000+? (affects pathfinding choice)
3. **Map Size:** Small (64x64) or large (256x256)?
4. **Terrain:** Flat, height levels, or smooth heightmap?
5. **Abilities:** Simple commands or complex spells?
6. **Fog of War:** Yes/no? What style?

**Decision:** Can answer these as we go, start with simple defaults

---

## Phase 1: Engine Foundation (Starting Now)

### Week 1: ECS Research & Implementation

**Tasks:**
1. Research ECS options
   - Create `docs/research/ecs-choice.md`
   - Options: bevy_ecs, hecs, specs, custom
   - Decide based on: performance, determinism support, ergonomics

2. Integrate chosen ECS
   - Add to `Cargo.toml`
   - Create `src/engine/ecs/` module structure
   - Basic entity spawn/despawn
   - Simple components (Transform, Velocity, Lifetime)

3. Test with simple example
   - Spawn 100 entities
   - Update system (move entities)
   - Render system (draw as colored cubes)

**Success Criteria:**
- Can spawn/despawn entities
- Components work correctly
- Systems update in order
- 1000+ entities at 60 FPS

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

## Immediate Action (Right Now!)

### Option A: Answer Design Questions First
We could discuss and decide on the open questions (unit count, map size, etc.) before starting implementation. This helps make better technical decisions.

### Option B: Start ECS Research
Jump right into researching ECS options and creating the research document. Get hands dirty with code.

### Option C: Start Procedural Modeling Research
Since this is the most unique system, start researching skin and subdivision algorithms. This might inform other decisions.

**Recommendation:** **Option B** (Start ECS Research)
- ECS is foundational - everything else builds on it
- We can answer design questions as we go
- Procedural modeling research can happen in parallel later
- Getting code working keeps motivation high

---

## What Would You Like to Do Next?

1. **Research ECS options** - I'll create `docs/research/ecs-choice.md`, analyze bevy_ecs/hecs/specs, make a recommendation

2. **Answer design questions** - We discuss and decide on unit count, map size, art style details

3. **Research procedural modeling** - Start with the unique system, understand skin/subdivision algorithms

4. **Something else** - You have a different priority or idea

Let me know and we'll proceed! 🚀
