# Flume Sugar - Game Design Document

**Last Updated:** 2026-02-17
**Status:** Planning

## Vision

A single-player Real-Time Strategy game in the style of StarCraft 2's campaign, featuring **procedurally generated models** and a **custom engine** built from the ground up in Rust. The game combines classic RTS mechanics with a unique procedural aesthetic and deep strategic gameplay.

## Core Pillars

1. **Procedural Aesthetics** - All models created through procedural modeling system (vertex graph → skin → subdivision)
2. **Strategic Depth** - Classic RTS mechanics: units, buildings, resources, tactical combat
3. **Visual Polish** - Dynamic lighting, shadows, smooth animations despite procedural nature
4. **Custom Tools** - Built-in map editor, debug tools, modding support
5. **Deterministic Simulation** - Support for replays and future multiplayer

## Genre & Style

- **Genre:** Real-Time Strategy (RTS)
- **Camera:** Perspective with low FOV, positioned high above ground
- **Player Count:** Single-player (potential future: 2-player co-op vs AI)
- **Visual Style:** Procedural geometric aesthetic (clean, stylized, angular forms)
- **Scope:** Campaign-focused with 10-15 missions

---

## Camera System (Technical Spec)

**Type:** Perspective projection with shallow field of view

**Characteristics:**
- **Low FOV** (15-25°) - Creates near-orthographic appearance with subtle depth
- **High altitude** - Camera positioned far above ground plane
- **Fixed pitch** - Always looking down at consistent angle (e.g., 45°)
- **Planar movement** - Moves on horizontal X/Z plane only (doesn't raise/lower with terrain)
- **Edge detection** - Clamps position to map bounds (can't scroll off edge)

**Controls:**
- **Pan:** Mouse edge scrolling OR WASD/arrow keys OR middle-mouse drag
- **Zoom:** Scroll wheel (adjusts camera height while maintaining angle)
- **Jump to:** Space bar jumps to last event (attack notification, etc.)

**Benefits:**
- Clear strategic overview of battlefield
- Minimal perspective distortion
- Consistent with RTS genre (StarCraft, Age of Empires style)
- Depth perception for unit stacking without confusion

---

## Core Gameplay Loop

1. **Gather Resources** - Workers collect minerals and gas from map nodes
2. **Build Base** - Construct buildings for production, research, defense
3. **Train Units** - Produce workers and military units
4. **Research Tech** - Unlock upgrades and new unit types
5. **Combat** - Attack enemies, defend base, complete objectives
6. **Progress** - Complete missions, unlock campaign progression

---

## Units & Buildings

### Worker Units
- Gather resources (minerals, gas)
- Construct buildings
- Repair structures (optional)
- **Example:** Builder Bot (procedural geometric humanoid)

### Military Units (Initial Set)
- **Scout** - Fast, weak, vision range
- **Infantry** - Basic melee/ranged ground unit
- **Heavy** - Tanky, slow, high damage
- **Artillery** - Long range, siege damage
- **Special** - Abilities (AoE, stealth, etc.)

### Buildings (Initial Set)
- **Command Center** - Main base, processes resources, trains workers
- **Barracks** - Trains infantry units
- **Factory** - Produces vehicles/heavy units
- **Tech Lab** - Unlocks upgrades and research
- **Turret** - Defensive structure with auto-attack

### Resources
- **Primary Resource** (Minerals) - Core currency, abundant
- **Secondary Resource** (Gas/Energy) - Advanced units/tech, limited nodes
- **Supply** - Population cap (built via supply buildings or inherent to other buildings)

---

## Procedural Modeling System (Core Feature)

### Workflow
Inspired by Blender's modifier workflow:

1. **Define Vertex Graph**
   - Place vertices (nodes) in 3D space
   - Connect vertices with edges
   - Assign properties (edge thickness, vertex scale)

2. **Apply Skin Modifier**
   - Creates cylindrical volume around each edge
   - Generates tube-like geometry
   - Handles branching at vertices
   - Configurable radius and resolution

3. **Apply Subdivision Modifier**
   - Smooths geometry (Catmull-Clark or Loop subdivision)
   - Increases polygon count for smooth curves
   - Configurable subdivision level (0-3)

4. **Generate Final Mesh**
   - Output triangle mesh for GPU rendering
   - Cache for static objects
   - Regenerate for animated objects

### Benefits
- **Small data size** - Store 20 vertices vs 10,000 triangle vertices
- **Consistent style** - All objects share same procedural aesthetic
- **Easy LOD** - Adjust subdivision level based on distance
- **Animation-friendly** - Animate source vertices, procedural mesh follows

### Visual Style
- Angular, geometric forms with smooth subdivisions
- Clean silhouettes
- Glowing edges or energy fields for accents
- Minimalist but recognizable unit shapes

---

## Graphics & Rendering

### Lighting
- **Directional Light** (Sun) - Primary scene illumination, casts shadows
- **Point Lights** (Torches, effects) - Local illumination, glowing objects
- **Ambient Lighting** - Soft fill light to prevent pure black shadows
- **Emissive Materials** (Optional) - Self-illuminated objects (energy cores, shields)

**NOT implementing:** Full global illumination (too complex for first iteration)

### Shadows
- **Dynamic Unit Shadows** - Real-time shadow casting from units
- **Building Shadows** - Either static baked or dynamic (depends on performance)
- **Shadow Technique** - Shadow mapping with PCF for soft edges
- **Shadow Target** - Shadows cast on terrain and other objects

### Visual Effects
- **Particle Systems** - Explosions, weapon fire, smoke, magic abilities
- **Post-Processing** - Bloom (glow), color grading, vignette (optional)
- **UI Effects** - Selection rings, health bars, damage numbers

---

## Animation System

### Approach
**Vertex Animation** on source vertices (before skin/subdivision):

1. Define keyframes for vertex positions in vertex graph
2. Interpolate between keyframes
3. Regenerate procedural mesh each frame (or cache frames)
4. Result: Smooth animation that works with procedural system

### Animation Types
- **Idle** - Slight hover/breathing movement
- **Walk/Run** - Locomotion cycles (legs, body sway)
- **Attack** - Wind-up, strike, recovery
- **Death** - Collapse or explode
- **Abilities** - Special move animations

### Optimization
- Cache subdivided meshes for animation frames (don't recalculate every time)
- Reduce animation quality for distant units (fewer frames, lower subdivision)
- Skip animations for off-screen units

---

## Deterministic Simulation

**Why:** Enable save/load and replay systems

### Requirements
1. **Fixed Timestep** - Update game logic at fixed rate (60 ticks/sec), decouple from frame rate
2. **Deterministic Math** - Same inputs always produce same outputs
   - Use seeded RNG (ChaCha or PCG)
   - Careful with floating-point (avoid platform differences)
   - Consider fixed-point math for critical systems
3. **Deterministic Iteration** - ECS systems iterate components in consistent order
4. **Command-Based Input** - Record commands (not states) for replays

### Physics Approach
**NO traditional physics engine** (they're often non-deterministic)

Instead:
- Simple collision detection (AABB, spheres, capsules)
- Pathfinding-based movement (not force-based)
- Deterministic collision resolution (consistent tie-breaking)
- Spatial partitioning for performance (grid or quadtree)

---

## Input & Controls

### Mouse
- **Left Click** - Select unit/building
- **Left Drag** - Box selection (multiple units)
- **Right Click** - Command (move, attack, gather depending on target)
- **Middle Mouse Drag** - Pan camera (alternative to edge scrolling)
- **Scroll Wheel** - Zoom in/out

### Keyboard
- **Camera:** WASD or Arrow Keys (pan), +/- (zoom)
- **Control Groups:** Ctrl+1-9 (assign), 1-9 (select), Double-tap (jump to)
- **Building/Training:** Q, W, E, R (context-sensitive hotkeys)
- **Abilities:** Q, W, E, R (unit abilities)
- **Select All:** Ctrl+A (select all units of type), Double-click (select all visible)
- **Special:** Space (jump to last alert), F1-F4 (saved camera positions)

---

## User Interface

### HUD Elements
- **Top Bar** - Resources (minerals, gas), supply (current/max)
- **Minimap** (corner) - Shows map, unit positions, fog of war, alerts
- **Unit Info Panel** (bottom-left) - Selected unit name, health, abilities
- **Command Card** (bottom-center/right) - Build options, unit queue, abilities
- **Alerts** (left edge) - Attack warnings, construction complete, etc.

### Menus
- **Main Menu** - New Game, Load Game, Settings, Map Editor, Quit
- **In-Game Menu** (ESC) - Resume, Settings, Save, Load, Quit to Menu
- **Victory/Defeat** - Stats, replay option, continue/retry

### UI Style
- Clean, minimal design matching procedural aesthetic
- Geometric shapes, glowing accents
- Semi-transparent panels
- Color-coded resources and alerts

---

## Map & Level System

### Map Structure
- **Tile-based grid** (e.g., 128x128 or 256x256 tiles)
- **Terrain types:** Ground (walkable), Cliff (blocks movement, blocks building), Water (decorative), Unbuildable
- **Resource nodes:** Mineral patches, gas geysers (limited capacity)
- **Spawn points:** Player start, enemy bases, neutral units

### Map Editor
- **In-game tool** or **separate mode**
- **Features:**
  - Paint terrain tiles
  - Place resource nodes
  - Set spawn points
  - Place pre-placed units/buildings
  - Define mission objectives (future)
  - Set triggers/scripts (future)
  - Save/load maps (JSON or binary)
  - Playtest directly from editor

---

## Save/Load System

### Save Game
**Contents:**
- Game state (all entities, components, resources)
- Map state (destroyed objects, fog of war)
- Player progress (mission number, unlocks)
- Timestamp, play duration
- **Format:** JSON (human-readable) or compressed binary (smaller)

### Replay
**Contents:**
- Initial game state + RNG seed
- Sequence of player commands with timestamps
- Much smaller than full save (only commands, not states)
- **Playback:** Deterministic simulation replays commands
- **Features:** Fast-forward, rewind (restart + fast-forward), pause, slow-motion

**File storage:** User directory (`~/.flume_sugar/saves/`, `~/.flume_sugar/replays/`)

---

## Settings & Persistence

### Graphics Settings
- Resolution (dropdown)
- Display mode (fullscreen, windowed, borderless)
- Shadow quality (off, low, medium, high)
- Effects quality (particle count, bloom)
- VSync (on/off)
- Frame rate cap (30, 60, 120, unlimited)

### Audio Settings
- Master volume
- Music volume
- SFX volume
- UI sounds (on/off)

### Gameplay Settings
- Camera speed (edge scroll sensitivity, keyboard pan speed)
- Hotkey customization (rebind keys)
- UI scale (80%, 100%, 120%)
- Mouse sensitivity
- Selection behavior (additive, replace)

### Persistent Data
- Campaign progress (missions completed, unlocks)
- Settings preferences
- Statistics (games played, units built, etc.)
- Achievements (future)

---

## Development Tools

### Debug Overlays
- **FPS/Frame Time Graph** - Performance monitoring
- **Profiler** - System execution times, drawcall count
- **Unit Count** - Entities, components, queries
- **Pathfinding Visualization** - Show paths, obstacles, flowfields
- **Collision Bounds** - Draw AABBs, spheres, grid cells
- **AI State Display** - Show unit states, targets, decisions

### In-Game Console
- Spawn units/buildings via commands (`spawn_unit warrior 10`)
- Add resources (`add_minerals 1000`)
- Toggle debug visualizations (`toggle_collision_bounds`)
- Set game speed (`game_speed 0.5` for slow-mo, `2.0` for fast)
- Teleport camera (`camera_jump 100 50`)

### Entity Inspector
- Select entity in game
- View all components and values
- Edit values in real-time (dev mode)
- Delete entities

---

## Development Phases

### Phase 1: Engine Foundation ✅ Started
- ✅ Basic rendering (cube demo)
- ⬜ ECS implementation
- ⬜ RTS camera system
- ⬜ Input handling
- ⬜ Debug tools (FPS, profiler, debug renderer)

### Phase 2: Procedural Modeling
- ⬜ Vertex graph system
- ⬜ Skin modifier
- ⬜ Subdivision modifier
- ⬜ Mesh generation pipeline
- ⬜ LOD system

### Phase 3: Core Rendering
- ⬜ Depth buffer
- ⬜ Instanced rendering
- ⬜ Basic lighting (directional)
- ⬜ Material system
- ⬜ Shadow system

### Phase 4: Movement & Interaction
- ⬜ Collision system
- ⬜ Pathfinding (A* or flowfield)
- ⬜ Unit selection (mouse picking, box select)
- ⬜ Unit movement (click-to-move)

### Phase 5: Gameplay Systems
- ⬜ Combat (health, damage, targeting)
- ⬜ Resources (gathering, spending)
- ⬜ Buildings (construction, production queues)
- ⬜ Basic AI (attack, gather, patrol)
- ⬜ UI/HUD

### Phase 6: Polish & Content
- ⬜ Animation system
- ⬜ Particle effects
- ⬜ Audio system
- ⬜ Multiple unit/building types
- ⬜ Map editor

### Phase 7: Campaign & Features
- ⬜ Mission system (objectives, progression)
- ⬜ Save/load
- ⬜ Replays
- ⬜ Settings system
- ⬜ Campaign content (missions, story)

---

## Future Considerations

### Co-op Multiplayer
- **Networking:** Lockstep networking (determinism helps!)
- **Mode:** 2 players vs AI
- **Architecture:** Peer-to-peer or client-server
- **Challenges:** Latency, synchronization, cheating prevention

### Modding Support
- **Data files:** JSON for units, buildings, abilities
- **Custom maps:** Map editor produces standard format
- **Scripts:** Lua or Rhai for mission scripting (future)
- **Asset replacement:** Custom models via vertex graphs

---

## Questions to Answer (Before Full Production)

1. **Art Style Details**
   - More geometric/angular or more organic/smooth?
   - Color palette (bright/saturated or dark/muted)?
   - Example: StarCraft 2 (detailed, organic) vs. Planetary Annihilation (geometric, clean)

2. **Unit Count**
   - Target max units on screen? (100? 500? 1000+?)
   - Affects optimization strategy and pathfinding choice

3. **Map Size**
   - Small skirmish maps (64x64 tiles, quick games)?
   - Large campaign maps (256x256 tiles, epic battles)?

4. **Mission Structure**
   - Linear campaign (mission 1 → 2 → 3)?
   - Branching choices (different paths)?
   - Difficulty selection per mission?

5. **Abilities**
   - Simple commands only (move, attack, stop)?
   - Complex spell-like abilities (AoE, stuns, buffs)?

6. **Terrain**
   - Flat terrain with height levels (like SC2)?
   - Smooth heightmap (like Total Annihilation)?
   - Pure flat (simplest)?

7. **Fog of War**
   - Hide unexplored areas (black)?
   - Show last-seen state (grayed out)?
   - Full visibility (no fog)?

8. **Win Conditions**
   - Destroy all enemy buildings (classic)?
   - Objective-based (capture points, escort, defend)?
   - Resource collection?
   - Time-based?

---

## Success Metrics

**Minimum Viable Product (3 months):**
- 2 unit types, 3 building types
- Resource gathering works
- Combat works (units attack and die)
- Win condition (destroy enemy base)
- Single playable map
- Procedural models look good

**Full Game (6-8 months):**
- 5+ unit types, 5+ building types
- 10+ campaign missions
- Polished visuals (lighting, shadows, animations, particles)
- Map editor
- Save/load and replay systems
- Balanced gameplay, satisfying combat

**Stretch Goals (9-12 months):**
- 15+ missions with story
- Co-op multiplayer
- Multiple factions (different unit types)
- Advanced AI opponents
- Modding tools and documentation

---

## Estimated Timeline

**Total: 6-8 months for full single-player campaign**

See `docs/research/engine-requirements.md` for detailed phase breakdown.

---

## Let's Build This! 🚀

Next steps: Answer design questions, begin ECS research, start Phase 1 implementation.
