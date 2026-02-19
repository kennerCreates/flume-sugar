# Pathfinding & Crowd Simulation Research

**Date:** 2026-02-19
**Status:** Research Complete — Ready to Implement
**Author:** Research pass for Flume Sugar RTS

---

## Problem Statement

RTS units must:
1. Navigate from A to B around static obstacles (terrain, buildings)
2. Not overlap with other units
3. Move as coherent groups without spreading into a thin line or bunching into a blob
4. Flow smoothly through chokepoints without gridlock
5. Handle groups crossing each other in the middle of the battlefield

The test scenario that validates all of this: **8 groups placed at corners and edges of the map, each given an order to move to the diametrically opposite position.** All groups cross the center simultaneously. Good crowd simulation means they pass through each other like two streams of water, not collide and freeze.

---

## Why Every Existing RTS Gets This Wrong

### A* Per Unit (StarCraft 1, Age of Empires)
Each unit finds its own shortest path independently. Works fine with 5 units.
With 200 units targeting the same destination cell, every unit's optimal path converges to the same narrow corridor. They form a queue-of-queues, jostling for position at every waypoint. Result: the infamous pathing pile-ups.

**Root cause:** A* treats other units as non-entities. No awareness of crowd state.

### Vanilla Flowfields (Supreme Commander, Planetary Annihilation)
One flowfield per destination, shared by all units in the group. Efficient, handles static obstacles beautifully. But:
- Units still physically overlap — the flowfield has no unit separation force
- All units flow to the *exact same cell*, causing destination compression
- Flowfield is static — doesn't react to where units currently *are*

**Root cause:** Flowfields model terrain cost, not crowd cost.

### Unit Separation Forces (StarCraft 2)
SC2 adds weak repulsion forces between nearby units on top of A*. Better than nothing, but:
- The forces are tuned to keep units "clumped tight" for visual clarity — which means they look good in a base but clog at chokepoints
- Force direction conflicts with path direction, causing jitter at high density
- Individual A* paths for hundreds of units is expensive

**Root cause:** Separation is bolted onto pathfinding rather than integrated.

### Formation Movement (Company of Heroes, Dawn of War)
Squad-level slot assignment, units move to fill slots. Clean for small squads.
Doesn't scale: a 200-unit formation hitting a narrow bridge either breaks or causes traffic jams as slots can't fit.

**Root cause:** Formation geometry doesn't adapt to topology dynamically.

---

## Proposed Architecture: Three-Layer Crowd System

The key insight: **pathfinding (macro), local avoidance (micro), and crowd density (feedback) should all be aware of each other.** Most RTS systems treat these as separate, non-communicating layers.

```
┌─────────────────────────────────────────────────────┐
│  Layer 1: Navigation Grid                           │
│  Static terrain walkability + building footprints  │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  Layer 2: Group Flowfield                           │
│  BFS/Dijkstra from goal, shared per group           │
│  Cost = terrain_cost + crowd_density_cost           │  ← NOVEL
└──────────────────────┬──────────────────────────────┘
                       │ desired_direction per unit
┌──────────────────────▼──────────────────────────────┐
│  Layer 3: ORCA Local Avoidance                      │
│  Per-unit velocity adjustment to avoid neighbors    │
│  Predictive: uses expected positions, not current   │  ← NOVEL
└──────────────────────┬──────────────────────────────┘
                       │ final_velocity
┌──────────────────────▼──────────────────────────────┐
│  ECS Movement System                                │
│  Apply final_velocity to Transform                  │
└─────────────────────────────────────────────────────┘
```

---

## Layer 1: Navigation Grid

A uniform grid aligned to the map's tile grid (same resolution).

```rust
struct NavCell {
    walkable: bool,
    terrain_cost: f32,   // 1.0 = normal, 2.0 = slow terrain, f32::MAX = wall
}

struct NavigationGrid {
    cells: Vec<NavCell>,
    width: u32,
    height: u32,
    cell_size: f32,      // world units per cell (e.g. 1.0)
}
```

**Dynamic obstacles** (units, buildings) are NOT baked into NavCell. They're handled by Layer 3 (ORCA). This is important — baking units into the grid causes the flowfield to steer into "ghost" positions from last frame.

**Buildings** are baked into `walkable` at construction time. Recalculate affected flowfields when a building is placed/destroyed.

---

## Layer 2: Group Flowfield

### Why Flowfields Over A*

| | A* Per Unit | Flowfield |
|---|---|---|
| 200 units, same goal | 200 searches | 1 search |
| Query cost per unit | O(path_len) waypoint tracking | O(1) array lookup |
| Works for groups | Poor — all find same path | Excellent |
| Works for individuals | Good | Good (deduplicate same goals) |
| Dynamic replanning | Per-unit, expensive | Recalculate once for group |

For an RTS, the common case is "selected group → move to point." Flowfields win decisively.

### Flowfield Computation

Not A* (start → goal). Instead: **multi-source BFS from the goal outward** (Dijkstra's).
This gives every cell the shortest distance to the goal, considering terrain cost.

```rust
struct FlowField {
    // Per-cell: direction vector to move toward goal (normalized, pre-computed)
    directions: Vec<Vec2>,
    // Integration field (distance to goal) — kept for density feedback
    integration: Vec<f32>,
    width: u32,
    height: u32,
    goal_cell: UVec2,
}
```

Gradient of `integration` field gives the direction vector. No per-unit pathfinding needed.

### The Novel Part: Density Feedback Cost

**Problem:** Standard flowfields route all units to the goal by the same shortest-cost path. If 200 units are in a 5-tile-wide corridor, they pile 40 deep. A path 3 tiles longer that's unoccupied is clearly better, but the flowfield doesn't know units are clogging the fast route.

**Solution:** Every N frames (e.g. every 8 frames = ~133ms at 60fps), recompute the flowfield with a **density surcharge** added to the integration cost:

```
effective_cost(cell) = terrain_cost(cell) + density_weight * unit_density(cell)
```

Where `unit_density(cell)` is the number of units currently in that cell (or nearby cells, smoothed).

**Effect:** The flowfield dynamically routes units away from congested areas, naturally spreading the group across all available width. Like water finding all available downhill paths simultaneously. No unit "decides" to take a side route — the gradient just changes to make side routes cheaper.

**Density map update:** Maintained by a separate spatial grid updated each frame (needed for ORCA anyway). Cost: O(unit_count) per frame to update.

**Tuning:** `density_weight` controls how aggressively units spread out. Too high = units take very roundabout routes. Too low = corridor pileups return. Start at 0.3–0.5 × terrain_cost of a normal cell.

### FlowField Cache

```rust
struct FlowFieldCache {
    // Map from goal cell → (FlowField, frame_computed, unit_count_at_time)
    fields: HashMap<UVec2, CachedFlowField>,
}
```

Groups with the same destination share a flowfield. Invalidate when:
- A building is placed/destroyed (walkability changes)
- Density has changed significantly (recompute with new density surcharge every N frames)

---

## Layer 3: ORCA Local Avoidance

### What Is ORCA

**Optimal Reciprocal Collision Avoidance** — a velocity-space algorithm from UNC Chapel Hill (van den Berg et al., 2011). Each agent computes a new velocity that:
- Avoids collision with all nearby agents within a time horizon
- Is as close as possible to the agent's desired (flowfield) velocity
- Takes mutual responsibility — each agent adjusts *half* the required avoidance

Result: no oscillation, no deadlock, mathematically optimal local behavior.

### Why ORCA Over Simple Repulsion Forces

| | Spring Forces | ORCA |
|---|---|---|
| Oscillation | Common (springs overshoot) | None (one-shot optimal) |
| Deadlock at density | Frequent | Rare |
| Units tunneling through each other | Possible at high speed | Impossible |
| Computational cost | O(n²) naive | O(n log n) with spatial grid |
| Tuning | Many spring constants | Just radius + time horizon |

### Predictive ORCA (Novel Enhancement)

Standard ORCA looks at current positions and velocities of neighbors. If a unit is heading toward you at speed, ORCA sees it when it's close and makes a sharp swerve.

**Enhancement:** Each unit extrapolates *expected* positions for neighbors using their current flowfield direction (not just current velocity). With a 1–2 second time horizon, units start adjusting their course before neighbors are close, creating smooth, gradual lane changes rather than sudden swerves.

```rust
// When computing ORCA constraints for neighbor:
// Use: expected_position = neighbor.position + neighbor.flowfield_dir * time_horizon * neighbor.speed
// Not just: neighbor.position + neighbor.velocity * time_horizon
```

This makes group-crossing events look like two streams merging and separating naturally.

### Spatial Grid for Neighbor Queries

ORCA only needs neighbors within `agent_radius * 2 + max_speed * time_horizon`.
For our units (~0.5 radius, ~3 speed, ~1s horizon): ~7 unit radius.
Spatial grid with 2.0 world-unit cells: check 4×4 = 16 cells per unit.

```rust
struct SpatialGrid {
    cells: Vec<SmallVec<[Entity; 8]>>,  // SmallVec to avoid heap alloc for sparse cells
    cell_size: f32,
    width: u32,
    height: u32,
}
```

Update: clear + re-insert all units each frame. O(n). Query: O(1) cell lookup + O(local_density) iteration.

---

## ECS Component Design

```rust
// --- Navigation Grid (Resource, singleton) ---
struct NavigationGrid { ... }
struct SpatialGrid { ... }           // for ORCA neighbor queries + density map

// --- Per-Group (Resource or tagged Entity) ---
struct UnitGroup {
    id: u32,
    member_entities: Vec<Entity>,
    goal: Option<Vec3>,
    flow_field: Option<Arc<FlowField>>,  // shared across members
}

// --- Per-Unit Components ---

/// Which group this unit belongs to (if any)
#[derive(Component)]
struct GroupMembership {
    group_id: u32,
}

/// Current movement goal and state
#[derive(Component)]
struct MovementOrder {
    destination: Vec3,
    arrived: bool,
}

/// Desired velocity from flowfield (set by FlowfieldSystem)
#[derive(Component)]
struct DesiredVelocity(Vec2);

/// Final velocity after ORCA adjustment (set by OrcaSystem, read by MoveSystem)
#[derive(Component)]
struct SteeringVelocity(Vec2);

/// Unit physical properties for ORCA
#[derive(Component)]
struct UnitAgent {
    radius: f32,      // physical radius (0.5 for our spheres)
    max_speed: f32,   // units/sec
}
```

### System Execution Order

```
Each frame:
1. SpatialGridUpdateSystem    — rebuild spatial grid from Transform positions
2. DensityMapUpdateSystem     — count units per nav cell (every 8 frames)
3. FlowFieldRecomputeSystem   — recompute flowfields if density changed or obstacles changed
4. DesiredVelocitySystem      — sample flowfield direction for each unit → write DesiredVelocity
5. OrcaSystem                 — compute avoidance → write SteeringVelocity
6. MoveSystem                 — apply SteeringVelocity to Transform, update Velocity component
7. ArrivalSystem              — check if unit reached destination, clear MovementOrder
```

All systems read from previous frame's Transform. No system reads from a Transform it just wrote. No dependencies within a single tick's transform data.

---

## Group Movement & Formation (Phase 2 Enhancement)

For the "group crossing" test, the above three layers are sufficient. But for polished RTS feel, a formation layer adds significant quality:

### Dynamic Formation Slots

The group has a *center* that follows the flowfield. Units occupy slots relative to the center:

```
Goal direction →
[  unit  ][  unit  ][  unit  ]
[  unit  ][  unit  ][  unit  ]
[  unit  ][  unit  ][  unit  ]
```

Slot assignment: use greedy nearest-slot matching (Hungarian algorithm is optimal but O(n³), greedy is good enough). Reassign slots every ~0.5 seconds or when the group changes direction significantly.

**Chokepoint adaptation:** When the flowfield indicates the path narrows (fewer navigable tiles ahead), gradually compress formation width and extend depth. Revert when space opens up. This eliminates the "clogged at doorway" problem.

The formation slot provides a *pull* force (soft spring toward slot position). ORCA still handles hard avoidance. The slot pull is weaker than ORCA — units temporarily leave slots to avoid each other but drift back when clear.

---

## Test Scene: The Crossing

**Setup:**
- 8 groups, ~250 units each (= 2000 total, matches current buffer)
- Groups at: NW, N, NE, E, SE, S, SW, W corners/edges
- Each group ordered to the diametrically opposite position
- Groups are color-coded by faction for visibility

**What to observe:**
- Groups should flow through each other in the center without gridlock
- Units should not overlap (ORCA guarantee)
- Groups should not "merge" — each unit should end up near the correct destination
- Chokepoint behavior: if walls are added, groups should queue and flow, not stop

**Debug visualizations needed:**
- Flowfield arrows (per-cell direction overlaid on ground)
- Density heatmap (cell color by unit count)
- ORCA velocity vector per unit (toggle with F4)
- Group membership coloring (already possible via Color component)

---

## Implementation Plan

### Sprint 1: Navigation Foundation
**Goal:** Groups can reach a destination, no avoidance yet

1. `NavigationGrid` struct + initialization from flat map (all walkable for now)
2. `FlowField` computation (Dijkstra's from goal)
3. `DesiredVelocitySystem`: sample flowfield at unit position, write to `DesiredVelocity`
4. `MoveSystem`: apply `DesiredVelocity` directly to position (replace current random movement)
5. `UnitGroup` resource: assign 8 groups, give each a goal
6. Test: 8 colored groups all moving to their destinations

**Deliverable:** Groups navigate to destination, units pile up at destination (acceptable for sprint 1)

### Sprint 2: ORCA Local Avoidance
**Goal:** Units don't overlap

1. `SpatialGrid` struct + per-frame update system
2. ORCA algorithm implementation (reference: RVO2 paper or `rvo2-rs` crate as reference, but implement from scratch for control)
3. `OrcaSystem`: query neighbors, compute avoidance velocity, write `SteeringVelocity`
4. Update `MoveSystem` to use `SteeringVelocity` instead of `DesiredVelocity`
5. Tune: unit radius, ORCA time horizon, max speed

**Deliverable:** Units in groups don't overlap. Crossing test shows streams passing through.

### Sprint 3: Density Feedback
**Goal:** Groups spread across available space, no corridor piling

1. `DensityMap` update system (every 8 frames)
2. Modify flowfield recomputation to include density surcharge
3. Tune density weight
4. Test: narrow corridor test — groups should use full corridor width

**Deliverable:** No more 1-unit-wide queues through corridors.

### Sprint 4: Formation + Arrival
**Goal:** Groups feel cohesive, stop cleanly at destination

1. Formation slot system for group-member offset assignments
2. Chokepoint detection from flowfield integration values
3. Dynamic slot compression/expansion
4. Arrival detection (all units within radius of goal)
5. Group disbanding (clear FlowField ref, let units idle)

**Deliverable:** Groups move like a unit, stop at destination cleanly.

### Sprint 5: Debug Visualization
1. Debug render: flowfield arrows (F5 toggle)
2. Debug render: density heatmap overlay
3. Debug render: ORCA velocity lines per unit
4. Debug overlay stats: pathfinding compute time, flowfield cache hits

---

## Crate Decisions

| Need | Decision | Rationale |
|------|----------|-----------|
| Flowfield BFS | Custom | Simple, 50 lines, full control for density integration |
| ORCA | Custom | Algorithm is straightforward; `rvo2-rs` exists as reference |
| Spatial grid | Custom | Already need one for rendering |
| Parallelism | `rayon` | Flowfield recompute per group can run in parallel |

The ORCA algorithm core is ~80 lines of math. Not worth adding a dependency that may not integrate cleanly with our ECS or spatial grid design.

---

## Complexity & Performance Budget

Target: 2000 units at 60 FPS on mid-range GPU.

| System | Complexity | Est. Cost |
|--------|-----------|-----------|
| Spatial grid rebuild | O(n) | ~0.2ms |
| Density map update (8-frame cadence) | O(n) | ~0.2ms amortized |
| FlowField recompute (128×128 map) | O(map_cells) | ~1ms amortized (shared per group) |
| Desired velocity sample | O(n) | ~0.05ms |
| ORCA (8 neighbors avg.) | O(n × k) | ~1–2ms |
| Move system | O(n) | ~0.1ms |

Total estimated: **~3ms** per frame for 2000 units. Well within 16ms budget.

**Rayon parallelism:** ORCA per-unit is embarrassingly parallel (units only read spatial grid, don't write to each other's state in the solve phase). Rayon par_iter over units should give near-linear scaling with core count.

---

## Key Papers & References

- **ORCA:** "Reciprocal n-Body Collision Avoidance" — van den Berg, Guy, Lin, Manocha (2011)
- **Flowfields for games:** Elijah Emerson's "Crowd Pathfinding and Steering Using Flow Field Tiles" (2013) — describes the approach used in Supreme Commander 2
- **Density-weighted navigation:** Related to "Social Force Model" — Helbing & Molnar (1995) — adapted for discrete flowfields
- **HPA*:** "Near Optimal Hierarchical Path-Finding" — Botea, Müller, Schaeffer (2004) — optional optimization for larger maps

---

## Decision: Implement This Architecture

**Rationale:**
- Flowfields handle the "group to destination" case perfectly (O(1) per unit after compute)
- ORCA handles unit separation without oscillation or deadlock
- Density feedback is the differentiator — no shipping RTS implements this at the crowd level
- All three layers fit cleanly into the existing ECS architecture
- No external dependencies required
- Deterministic: flowfield is deterministic (BFS), ORCA is deterministic (algebraic, no RNG), spatial grid iteration order is deterministic if sorted by entity ID

**What we're NOT doing (and why):**
- No navmesh: tile grid is simpler, sufficient for an RTS with discrete tile terrain, and easier to debug
- No A* per unit: only for individual units outside a group (workers, scouts) — may add later
- No rigid formations: too brittle at chokepoints. Loose formation with slot attraction is strictly better
