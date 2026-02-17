# Engine Requirements Analysis

**Date:** 2026-02-17
**Status:** Planning
**Last Updated:** 2026-02-17

## Problem Statement

Build a custom game engine in Rust capable of supporting an RTS game with:
- Hundreds of units with independent behaviors
- Procedural modeling and animation
- Dynamic lighting and shadows
- Deterministic simulation for replays
- Built-in development tools

## Testing Strategy

**Approach:** Balanced testing - test critical systems, skip trivial code, maintain velocity.

**What to Test:**
- ✅ Core algorithms (pathfinding, collision detection, subdivision)
- ✅ Determinism (critical for replays - same inputs = same outputs)
- ✅ Data structures (ECS queries, spatial partitioning)
- ✅ Serialization (save/load correctness)
- ✅ Performance benchmarks (frame time with N units)

**What NOT to Test:**
- ❌ Trivial code (getters, setters)
- ❌ Rendering output (use visual testing)
- ❌ UI layout (manual testing)
- ❌ Experimental/prototype code (test when stable)

**Testing Tools:**
- Built-in Rust `#[test]` for unit tests
- `criterion` for performance benchmarks
- `proptest` for property-based testing (optional)
- Manual testing for graphics/gameplay

**Workflow:** Implement → Manual test → Add unit tests for algorithms → Add benchmarks if critical → Refactor with confidence

---

## Required Engine Systems

### 1. Core Engine Architecture

**ECS (Entity Component System)**
- **Why:** Efficient management of hundreds/thousands of game objects
- **Requirements:**
  - Component registration system
  - System execution order (deterministic!)
  - Entity queries (all units with Health component, etc.)
  - Fast iteration over components
  - Deterministic iteration order (critical for replays)
- **Options to research:**
  - bevy_ecs (standalone Bevy ECS, battle-tested)
  - hecs (simple, fast, minimal)
  - specs (mature, feature-rich, used in Amethyst)
  - Custom implementation (maximum control)
- **Testing:**
  - ✅ Integration tests for entity lifecycle, queries
  - ✅ Determinism tests (same operations = same state)
  - ✅ Benchmarks for query performance

**Priority:** CRITICAL - Foundation for all gameplay

**Research Doc:** `docs/research/ecs-choice.md` (to be created)

---

### 2. Rendering System (ENGINE)

**Current Status:** Basic wgpu pipeline with single rotating cube

**Needed Additions:**

**a) Camera System**
- Perspective projection with low FOV (15-25°)
- View matrix calculation
- Position on horizontal plane (X/Z movement only)
- Map bounds clamping (edge detection)
- Zoom in/out (adjust camera height)
- Screen-to-world ray casting (for mouse picking)
- **Testing:** ✅ Unit tests for ray casting, bounds clamping

**b) Instanced Rendering**
- Draw many copies of same mesh efficiently
- Per-instance data (transform, color/material ID)
- Batch similar units together
- **Testing:** ✅ Benchmark instance count vs frame time

**c) Material System**
- Different material types (unlit, lit, emissive)
- Shader variants for different materials
- Material properties (color, roughness, metallic, emission)
- **Testing:** Manual visual testing

**d) Lighting**
- Directional light (sun) with direction and color
- Point lights (torches, effects) with position, color, radius
- Light uniform buffers (send to GPU)
- Per-fragment lighting calculations
- **Testing:** Manual visual testing

**e) Shadow System**
- Shadow map generation (render depth from light's view)
- PCF (Percentage Closer Filtering) for soft shadows
- Shadow receiving on all meshes
- Cascaded shadow maps (optional, for large view distances)
- **Testing:** Manual visual testing, performance benchmarks

**f) Depth Buffer**
- Z-testing for correct occlusion (closer objects hide farther ones)
- Depth attachment in render pass
- **Testing:** Manual visual testing

**Priority:** HIGH - Core engine, reusable across games

**Research Doc:** `docs/research/camera-system.md` (to be created)

---

### 3. Procedural Modeling System (ENGINE)

**Most Unique System - Defines visual style**

**Requirements:**
- Vertex graph definition (nodes with positions, edges connecting them)
- Skin modifier implementation (create cylindrical volume around edges)
- Subdivision surface modifier (Catmull-Clark or Loop subdivision)
- Mesh generation pipeline: Graph → Skin → Subdivide → Triangle mesh
- LOD system (different subdivision levels based on distance)
- Mesh caching (don't regenerate every frame unless animated)

**Components:**
- `VertexGraph` struct (nodes: Vec<Vec3>, edges: Vec<(u32, u32)>, properties)
- `SkinModifier` (edge radius, radial segments, end caps)
- `SubdivisionModifier` (subdivision level 0-3, boundary handling)
- `ProceduralMesh` (final triangle mesh for rendering)

**Research Topics:**
- Skin modifier algorithm (cylinder generation, branching, caps)
- Subdivision surface algorithms (Catmull-Clark for quads, Loop for triangles)
- Efficient mesh generation (minimize allocations)
- Memory management for generated meshes
- Caching strategy for static/animated meshes

**Testing:**
- ✅ Unit tests for subdivision correctness (known input → expected output)
- ✅ Unit tests for skin generation (edge count → vertex count)
- ✅ Property tests (generated mesh is always manifold, no holes)
- ✅ Benchmarks for mesh generation time
- Manual visual testing for appearance

**Priority:** HIGH - Unique system, defines art style, affects everything

**Research Doc:** `docs/research/procedural-modeling.md` (to be created - HIGH PRIORITY)

---

### 4. Animation System (ENGINE)

**Requirements:**
- Animate source vertices in vertex graph (before skin/subdivision)
- Regenerate procedural mesh from animated graph
- Keyframe animation (vertex position over time)
- Animation blending (walk to run transition)
- Animation state machine (idle → walk → attack → idle)
- Skeletal-like system (groups of vertices move together as "bones")

**Components:**
- `Animation` struct (keyframes: Vec<Frame>, duration, loop)
- `Frame` struct (vertex_positions: Vec<Vec3>, timestamp)
- `AnimationController` component (current animation, state machine, blend factor)
- `AnimationClip` resource (reusable animation data)

**Optimization:**
- Cache subdivided meshes for each animation frame
- Use GPU skinning if possible (vertex shader animation)
- LOD: reduce animation quality/framerate for distant units

**Testing:**
- ✅ Unit tests for keyframe interpolation
- ✅ Unit tests for state machine transitions
- ✅ Benchmarks for animation update cost
- Manual testing for smooth appearance

**Priority:** MEDIUM - Needed for polish, but simple animations work initially

**Research Doc:** `docs/research/animation-system.md` (to be created)

---

### 5. Input System (ENGINE)

**Requirements:**
- Mouse input (position, buttons, drag, wheel)
- Keyboard input (key down, up, held)
- Input state tracking (just pressed this frame? held?)
- Input mapping (rebindable keys)
- Input buffering (queue commands for next frame)

**Components:**
- `InputManager` resource (singleton, tracks all input state)
- `MouseState` struct (position, buttons[3], wheel_delta)
- `KeyboardState` struct (keys: HashMap<KeyCode, bool>)
- Process winit events into input state

**Testing:**
- ✅ Unit tests for input state tracking (press, release, held)
- Manual testing for responsiveness

**Priority:** HIGH - Needed for any interaction

---

### 6. UI System (ENGINE with GAME-specific layouts)

**Engine Components:**
- UI rendering (2D shapes, text, images on screen overlay)
- UI layout system (anchoring, scaling, flex-box-like)
- UI interaction (clickable buttons, hover states, text input)
- UI theming/styling

**Options:**
- **egui** (immediate mode GUI, easy integration, good for tools)
  - Pros: Fast to iterate, built-in widgets, dev tools support
  - Cons: Immediate mode (rebuild every frame), distinctive look
- **Custom UI** (more control, more work, fits game aesthetic)
  - Pros: Perfect fit for game style, maximum control
  - Cons: Time investment, need to build everything
- **iced** (Elm-like declarative UI, native Rust)
  - Pros: Clean architecture, reactive
  - Cons: Might be overkill, less game-focused

**Recommendation:** Start with egui for dev tools, build custom for game UI later

**Game-Specific UI Elements:**
- Minimap widget (shows map, units, fog of war)
- Resource display (minerals, gas, supply)
- Unit info panel (name, health, abilities)
- Command card (build options, unit production queue)

**Testing:**
- Manual testing for functionality and appearance

**Priority:** MEDIUM-HIGH - Need basic UI soon, can iterate

**Research Doc:** `docs/research/ui-system.md` (to be created)

---

### 7. Physics/Collision System (ENGINE)

**NOT using traditional physics engine** (determinism requirement)

**Requirements:**
- Collision shapes (AABB, sphere, capsule, OBB optional)
- Collision detection (broad phase + narrow phase)
- Deterministic collision resolution (fixed-point or controlled float)
- Spatial partitioning (grid or quadtree) for performance
- No velocity/forces (use pathfinding for movement)

**Components:**
- `Collider` component (shape type, dimensions, offset)
- `CollisionWorld` resource (spatial grid, queries)
- Collision events (via ECS events or callbacks)

**Algorithms:**
- Broad phase: Spatial grid (divide map into cells, only check nearby)
- Narrow phase: AABB-AABB (simple rect overlap), Sphere-Sphere (distance check)

**Testing:**
- ✅ Unit tests for collision detection (known shapes → collision yes/no)
- ✅ Property tests (collision is commutative: A hits B ↔ B hits A)
- ✅ Determinism tests (same positions = same collisions)
- ✅ Benchmarks for collision queries

**Priority:** MEDIUM - Needed for unit interactions, building placement

**Research Doc:** `docs/research/collision-system.md` (to be created)

---

### 8. Pathfinding System (ENGINE)

**Requirements:**
- Grid-based or navmesh pathfinding
- A* or flowfield algorithm
- Dynamic obstacles (units, buildings block paths)
- Group movement (formations, avoid bunching)
- Local collision avoidance (steering behaviors)

**Components:**
- `Pathfinder` resource (finds paths, caches navigation data)
- `NavigationGrid` resource (walkable tiles, costs, obstacles)
- `Path` component (list of waypoints to follow)
- `Movement` component (current velocity, target waypoint, speed)

**Options:**
- **A*** - Best for small unit counts, individual paths
  - Pros: Simple, well-understood, good for varied paths
  - Cons: Expensive for many units finding paths simultaneously
- **Flowfields** - Best for large unit counts moving to same goal
  - Pros: Many units share one flowfield, very efficient
  - Cons: Less flexible for individual unit behavior
- **Hybrid** - Flowfields for groups, A* for individuals

**Testing:**
- ✅ Unit tests for A* correctness (finds shortest path)
- ✅ Unit tests for obstacle avoidance
- ✅ Determinism tests (same grid + positions = same path)
- ✅ Benchmarks for pathfinding speed (various map sizes)

**Priority:** HIGH - Essential for RTS movement

**Research Doc:** `docs/research/pathfinding.md` (to be created - HIGH PRIORITY)

---

### 9. AI System (GAME)

**Game-specific logic, but might extract patterns to engine**

**Requirements:**
- State machines (idle, move, attack, gather, flee)
- Target acquisition (find enemies in range, prioritize)
- Attack behavior (attack-move, hold position, focus fire)
- Gather behavior (find resources, return to base, auto-resume)
- Formation movement (maintain positions relative to group)

**Components:**
- `AIState` component (current state enum)
- `AIBehavior` component (state machine logic, timers)
- `AttackTarget` component (current enemy target)
- `GatherTarget` component (current resource node)
- `Formation` component (offset within formation)

**Testing:**
- ✅ Unit tests for state machine transitions
- ✅ Determinism tests (same situation = same decision)
- Manual gameplay testing

**Priority:** MEDIUM - Needed for gameplay, but simple AI works initially

---

### 10. Resource Management System (ENGINE + GAME)

**Engine Components** (reusable asset loading):
- Asset loading (load files from disk or embedded)
- Asset caching (don't load same file twice)
- Asset hot-reloading (reload when file changes - dev only)
- Resource handles (ref-counted, typed IDs)
- Asset types: ProceduralMeshData, Textures (future), Audio (future)

**Game Components** (RTS resources):
- Player resources (minerals, gas, supply used/max)
- Resource nodes (harvestable map objects, capacity, regen)
- Resource gathering logic (harvest rate, carry capacity)

**Testing:**
- ✅ Unit tests for asset caching (load once, return cached)
- ✅ Unit tests for handle validity
- Manual testing for hot-reload

**Priority:** MEDIUM (asset loading HIGH, gameplay resources MEDIUM)

---

### 11. Audio System (ENGINE)

**Requirements:**
- Load audio files (WAV, OGG, MP3)
- Play sounds (one-shot, looping)
- 3D positional audio (optional - might be overkill for RTS)
- Volume control per sound and globally
- Music playback with crossfade

**Options:**
- **rodio** (simple, pure Rust, easy to use)
- **kira** (more features, game-focused, better control)
- **Custom OpenAL/FMOD wrapper** (maximum control, more work)

**Testing:**
- Manual testing (does it sound right?)

**Priority:** LOW - Polish feature, not needed for core gameplay

---

### 12. Particle System (ENGINE)

**Requirements:**
- Emit particles (position, velocity, lifetime, size, color)
- Update particles (gravity, drag, fade, size change over time)
- Render particles (billboards facing camera, GPU instancing)
- Particle effects (explosions, smoke, magic, building destruction)

**Components:**
- `ParticleEmitter` component (spawn rate, initial properties)
- `ParticleBuffer` (GPU buffer of particle instances)
- `ParticleSystem` resource (updates all particles)

**Testing:**
- ✅ Benchmarks for particle count vs frame time
- Manual visual testing

**Priority:** LOW-MEDIUM - Visual polish, not essential for prototype

---

### 13. Debug & Profiling Tools (ENGINE)

**Requirements:**
- FPS counter and frame time graph
- System execution time breakdown (ECS system timings)
- Memory usage tracking
- Debug visualizations (collision bounds, paths, AI states)
- In-game console (spawn entities, run commands, set variables)
- Entity inspector (view/edit component data live)

**Components:**
- `DebugRenderer` (draw lines, boxes, spheres, text in 3D world)
- `Profiler` (track timing data, store history)
- `Console` (command parser, command registry)
- Debug UI overlay (using egui)

**Testing:**
- Manual testing (does it help debug?)

**Priority:** HIGH - Essential for development, implement early alongside features

---

### 14. Map/Level System (ENGINE + GAME)

**Engine Components:**
- Tile-based map representation (2D grid of tiles)
- Map loading/saving (JSON or binary format)
- Terrain rendering (flat tiles or heightmap)
- Map bounds and edge detection

**Game Components:**
- Tile types (ground, cliff, water, unbuildable)
- Resource node placement data
- Spawn point definitions
- Objective markers and triggers

**Testing:**
- ✅ Unit tests for map serialization (save → load = same map)
- Manual testing for editor workflow

**Priority:** MEDIUM - Needed for structured levels, can use code-gen initially

---

### 15. Save/Load System (ENGINE)

**Requirements:**
- Serialize entire game state to file
- Deserialize game state from file
- Save format (JSON for debugging, binary for size)
- Compression (optional, for large saves)
- Version handling (detect old save format, migrate or error)

**Components:**
- `SaveGame` struct (contains all relevant state)
- Serialization traits for components (derive Serialize/Deserialize)
- File I/O with error handling

**Testing:**
- ✅ Unit tests for save → load round-trip
- ✅ Unit tests for version detection
- Manual testing with real game states

**Priority:** LOW - Polish feature, add after core gameplay works

---

### 16. Replay System (ENGINE/GAME)

**Requirements:**
- Record all player commands with timestamps
- Record initial game state + RNG seed
- Playback commands at exact timestamps (deterministic simulation)
- Fast-forward, rewind (restart from beginning, fast-forward to position)
- Pause, slow-motion
- Save replay to file (much smaller than save game)

**Requires:**
- Deterministic simulation (fixed timestep, deterministic RNG, deterministic ECS)
- Command pattern for all player actions

**Testing:**
- ✅ Determinism tests (replay produces exact same result)
- ✅ Round-trip test (play game, save replay, playback = same outcome)

**Priority:** LOW - Advanced feature, requires determinism foundation

**Research Doc:** `docs/research/determinism-and-replays.md` (to be created)

---

### 17. Settings System (ENGINE)

**Requirements:**
- Load/save settings to file (JSON in user directory)
- Graphics settings (resolution, fullscreen, quality levels)
- Audio settings (master/music/sfx volume)
- Gameplay settings (camera speed, hotkeys, UI scale)
- Apply settings at runtime (change resolution without restart)

**Testing:**
- ✅ Unit tests for settings serialization
- Manual testing for each setting

**Priority:** MEDIUM - Quality of life, important for final game

---

### 18. Deterministic Simulation (ENGINE)

**Critical for replay system**

**Requirements:**
- Fixed timestep update (e.g. 60 ticks/sec, independent of frame rate)
- Deterministic math (consistent across platforms)
  - Use controlled floating-point ops OR fixed-point math
  - Avoid platform-specific math functions
- Deterministic RNG (ChaCha or PCG with seed)
- Deterministic component iteration order (sorted by entity ID)
- Deterministic collision resolution (no random tie-breaking)

**Challenges:**
- Floating-point math can vary across CPUs/compilers
- Solutions:
  - Use fixed-point math for critical calculations
  - OR carefully control FP operations (avoid denormals, use consistent rounding)
  - Test determinism frequently

**Testing:**
- ✅ Critical: Determinism tests (run same inputs 1000 times, verify identical output)
- ✅ Cross-platform determinism tests (if targeting multiple platforms)

**Priority:** MEDIUM-HIGH - Foundation for replays, affects architecture decisions

---

## Implementation Priority & Timeline

### Phase 1: Foundation (Weeks 1-3)
**Goal:** Navigate a 3D world, spawn entities, see debug info

1. **ECS System** - Research options, implement or integrate
2. **Camera System** - RTS-style perspective camera with edge scrolling
3. **Input System** - Mouse and keyboard handling
4. **Debug Tools** - FPS counter, debug line rendering, basic profiler

**Deliverable:** Free camera flying around, press key to spawn colored cubes, see FPS

---

### Phase 2: Rendering Improvements (Weeks 4-6)
**Goal:** Render hundreds of objects efficiently with proper depth and lighting

5. **Depth Buffer** - Proper 3D rendering with Z-testing
6. **Instanced Rendering** - Draw many objects efficiently
7. **Basic Lighting** - Directional light, simple shading
8. **Material System** - Different surface appearances

**Deliverable:** 500 cubes with different materials, lit by sun, smooth 60 FPS

---

### Phase 3: Procedural Modeling (Weeks 7-10)
**Goal:** Create and render procedural models

9. **Vertex Graph System** - Define source geometry (nodes + edges)
10. **Skin Modifier** - Create volume from edges (cylindrical tubes)
11. **Subdivision Modifier** - Smooth geometry (Catmull-Clark or Loop)
12. **Mesh Generation Pipeline** - Complete procedural workflow
13. **LOD System** - Different detail levels based on distance

**Deliverable:** Procedural unit models (simple bot, building shapes) rendered in game

---

### Phase 4: Movement & Interaction (Weeks 11-13)
**Goal:** Select and move units around map

14. **Collision System** - Basic shapes, detection, spatial grid
15. **Pathfinding** - A* implementation with obstacle avoidance
16. **Unit Selection** - Mouse picking, box selection, control groups
17. **Unit Movement** - Click-to-move with pathfinding

**Deliverable:** Select units with mouse, right-click to move, units navigate around obstacles

---

### Phase 5: Gameplay Systems (Weeks 14-18)
**Goal:** Playable RTS prototype

18. **Combat System** - Health, damage, attack range, targeting
19. **Resource System** - Gather minerals/gas, spend on production
20. **Building System** - Construct buildings, production queues
21. **Basic AI** - Unit behaviors (attack, gather, patrol)
22. **UI System** - HUD (minimap, resources, unit info, command card)

**Deliverable:** Build workers, gather resources, build army, attack enemy base

---

### Phase 6: Animation & Polish (Weeks 19-22)
**Goal:** Polished, animated gameplay

23. **Animation System** - Vertex animation for procedural meshes
24. **Particle Effects** - Weapon fire, explosions, abilities
25. **Shadow System** - Dynamic shadows from units and buildings
26. **Audio System** - Sound effects and music

**Deliverable:** Animated units fighting with visual/audio feedback

---

### Phase 7: Content & Tools (Weeks 23-26)
**Goal:** Complete game experience with variety

27. **Map Editor** - Create custom maps in-game
28. **Settings System** - Graphics, audio, gameplay preferences
29. **Save/Load** - Persistent game state
30. **Multiple Unit/Building Types** - Content variety (5+ types each)

**Deliverable:** Multiple maps, different unit compositions, balanced gameplay

---

### Phase 8: Advanced Features (Weeks 27+)
**Goal:** Full-featured RTS

31. **Deterministic Simulation** - Fixed timestep, deterministic RNG
32. **Replay System** - Record and playback matches
33. **Campaign/Mission System** - Objectives, progression, unlocks
34. **Advanced AI** - Smarter tactics, build orders
35. **Performance Optimization** - Handle 1000+ units smoothly

**Deliverable:** Campaign with missions, replay viewer, optimized performance

---

## Next Immediate Steps

1. ✅ **Create GAME_DESIGN.md** - Document game vision (DONE)
2. ✅ **Create engine-requirements.md** - This document (DONE)
3. ⬜ **Answer design questions** (see GAME_DESIGN.md questions section)
4. ⬜ **Research ECS options** - Create `docs/research/ecs-choice.md`
5. ⬜ **Research camera system** - Create `docs/research/camera-system.md`
6. ⬜ **Research procedural modeling** - Create `docs/research/procedural-modeling.md` (HIGH PRIORITY)
7. ⬜ **Begin Phase 1 implementation** - ECS integration

---

## Success Criteria

**Short-term (3 months):**
- Working ECS with hundreds of entities
- Procedural modeling pipeline working
- Units can move and fight
- Basic RTS gameplay loop functional

**Long-term (6-8 months):**
- Playable campaign with missions
- Polished visuals (lighting, shadows, animations)
- Map editor for custom content
- Performance: 500+ units at 60 FPS

---

## Conclusion

This is an ambitious but achievable project. The key to success:

1. **Incremental development** - Small, testable milestones
2. **Engine/game separation** - Reusable systems from day one
3. **Documentation** - Avoid re-research, preserve context
4. **Balanced testing** - Test critical code, maintain velocity
5. **Visual feedback** - See progress regularly (motivation!)
6. **Focus on unique systems** - Procedural modeling is your differentiator

The procedural modeling system is the most innovative aspect - prioritize research and implementation of that system early. It will define the visual identity of your game.

**Estimated timeline:** 6-8 months for a complete, playable RTS with procedural aesthetics.

**Let's build something amazing! 🚀**
