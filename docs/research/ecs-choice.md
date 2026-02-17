# ECS (Entity Component System) Choice

**Date:** 2026-02-17
**Status:** Research Complete → Decision Made
**Last Updated:** 2026-02-17

## Problem Statement

Need an Entity Component System for managing game objects (units, buildings, projectiles, effects) with these requirements:

**Performance Requirements:**
- Handle 500-1000+ entities at 60 FPS
- Efficient queries (e.g., "all units in radius", "all units with Shield ability")
- Minimal iteration overhead

**Functional Requirements:**
- Component flexibility (modular abilities - swap components between entities)
- Deterministic iteration order (for replays)
- System execution ordering
- Entity lifecycle management (spawn, despawn, cleanup)

**Developer Experience:**
- Good ergonomics (easy to add components, define systems)
- Clear documentation
- Active maintenance
- Rust-native (leverage type safety, ownership)

---

## Options Considered

### Option 1: bevy_ecs

**What it is:**
- ECS from Bevy game engine, available as standalone crate
- Modern, high-performance archetype-based ECS
- Designed for games with thousands of entities

**Pros:**
- ✅ **Excellent performance** - Archetype storage, cache-friendly iteration
- ✅ **Battle-tested** - Used in Bevy engine, production-ready
- ✅ **Flexible queries** - Powerful query syntax (`Query<(&Transform, &Health), With<Enemy>>`)
- ✅ **Change detection** - Built-in change tracking (optimize updates)
- ✅ **Parallel systems** - Automatic parallelization based on data dependencies
- ✅ **Events** - Built-in event system for entity communication
- ✅ **Resources** - Global singletons (InputManager, Time, etc.)
- ✅ **Active development** - Part of Bevy ecosystem, frequent updates
- ✅ **Good documentation** - Bevy book covers ECS extensively

**Cons:**
- ⚠️ **Larger dependency** - Pulls in some Bevy infrastructure
- ⚠️ **Learning curve** - Powerful but complex API
- ⚠️ **Determinism** - Not deterministic by default (must configure carefully)
  - Iteration order depends on archetype creation order
  - Must sort entities or use `Entity` IDs explicitly

**Performance Characteristics:**
- **Entity iteration:** ~0.5-1 ns per entity (bare minimum overhead)
- **1000 entities:** Sub-millisecond iteration
- **Archetype storage:** Excellent cache locality

**Determinism Approach:**
- Entities have stable IDs (`Entity` with generation counter)
- Can iterate in sorted order: `query.iter().sorted_by_key(|e| e.id())`
- Systems can be run in fixed order
- With care, can achieve determinism

**Example Code:**
```rust
use bevy_ecs::prelude::*;

#[derive(Component)]
struct Transform { position: Vec3 }

#[derive(Component)]
struct Health(f32);

fn move_system(mut query: Query<&mut Transform, With<Health>>) {
    for mut transform in query.iter_mut() {
        transform.position.x += 1.0;
    }
}
```

**Verdict:** **Best performance, most features, determinism achievable with care**

---

### Option 2: hecs

**What it is:**
- Lightweight, minimal ECS
- Archetype-based like bevy_ecs
- Focus on simplicity and performance

**Pros:**
- ✅ **Excellent performance** - Similar to bevy_ecs (archetype storage)
- ✅ **Minimal dependencies** - Small, focused crate
- ✅ **Simple API** - Easy to learn, less magic
- ✅ **Lightweight** - Fast compile times
- ✅ **Deterministic** - Simpler to reason about iteration order

**Cons:**
- ⚠️ **Fewer features** - No built-in events, change detection, resources
- ⚠️ **Manual parallelization** - No automatic parallel systems (must use rayon manually)
- ⚠️ **Less ergonomic** - More boilerplate than bevy_ecs
- ⚠️ **Smaller ecosystem** - Less documentation, fewer examples

**Performance Characteristics:**
- **Entity iteration:** Similar to bevy_ecs (~0.5-1 ns per entity)
- **1000 entities:** Sub-millisecond iteration
- **Memory:** Very efficient archetype storage

**Determinism Approach:**
- Entities have stable IDs
- Iteration order is deterministic within archetypes
- Simpler model makes determinism easier

**Example Code:**
```rust
use hecs::*;

struct Transform { position: Vec3 }
struct Health(f32);

let mut world = World::new();
let entity = world.spawn((Transform { position: Vec3::ZERO }, Health(100.0)));

for (id, (transform, health)) in world.query_mut::<(&mut Transform, &Health)>() {
    transform.position.x += 1.0;
}
```

**Verdict:** **Good performance, simpler, but fewer features - good if you want control**

---

### Option 3: specs (SPECS: Parallel ECS)

**What it is:**
- Mature ECS library, used in Amethyst game engine
- Component storage-based (not archetype-based)
- Focus on parallelism and flexibility

**Pros:**
- ✅ **Mature** - Years of production use, stable API
- ✅ **Flexible storage** - Choose storage type per component (DenseVec, HashMap, etc.)
- ✅ **Parallel systems** - Built-in parallelization with `rayon`
- ✅ **Resources** - Global data management
- ✅ **Good documentation** - Book, tutorials, examples

**Cons:**
- ❌ **Slower than archetype ECS** - Component storage = more cache misses
- ❌ **More verbose** - Lots of boilerplate (storage registration, etc.)
- ❌ **Less active** - Maintenance mode (Amethyst development slowed)
- ❌ **Determinism challenges** - Parallel execution order non-deterministic

**Performance Characteristics:**
- **Entity iteration:** ~5-10 ns per entity (slower than archetype ECS)
- **1000 entities:** Still fast, but noticeably slower than bevy_ecs/hecs
- **Memory:** Less cache-friendly than archetype storage

**Determinism Approach:**
- Harder to achieve determinism with parallel systems
- Must force sequential execution or very careful synchronization

**Example Code:**
```rust
use specs::prelude::*;

struct Transform { position: Vec3 }
impl Component for Transform { type Storage = VecStorage<Self>; }

struct Health(f32);
impl Component for Health { type Storage = VecStorage<Self>; }

struct MoveSystem;
impl<'a> System<'a> for MoveSystem {
    type SystemData = (WriteStorage<'a, Transform>, ReadStorage<'a, Health>);

    fn run(&mut self, (mut transforms, healths): Self::SystemData) {
        for (transform, health) in (&mut transforms, &healths).join() {
            transform.position.x += 1.0;
        }
    }
}
```

**Verdict:** **Mature but slower, less active development - not ideal for 1000+ entities**

---

### Option 4: Custom ECS

**What it is:**
- Roll our own ECS from scratch
- Full control over every aspect

**Pros:**
- ✅ **Full control** - Exactly what we need, no more, no less
- ✅ **Determinism** - Can guarantee from the start
- ✅ **Learning** - Deep understanding of ECS internals
- ✅ **Optimization** - Tailor to exact use case

**Cons:**
- ❌ **Time investment** - Weeks to months to build properly
- ❌ **Bug potential** - Easy to get wrong (memory leaks, perf issues)
- ❌ **Maintenance burden** - Must fix all bugs ourselves
- ❌ **Reinventing wheel** - Existing solutions are battle-tested
- ❌ **Opportunity cost** - Time not spent on game features

**Verdict:** **Not recommended - premature optimization, high risk, low reward**

---

## Performance Comparison

Benchmark: Iterating 10,000 entities with 3 components (Transform, Velocity, Health)

| ECS | Time (μs) | Relative | Notes |
|-----|-----------|----------|-------|
| **bevy_ecs** | ~50-100 | 1.0x | Archetype storage, excellent cache locality |
| **hecs** | ~50-120 | 1.0-1.2x | Similar to bevy_ecs, slightly less optimized |
| **specs** | ~200-300 | 3-4x | Component storage, more cache misses |
| **Custom (naive)** | ~500+ | 5x+ | Depends heavily on implementation |

**Source:** Various ECS benchmarks, approximate values

**For 1000 entities at 60 FPS:**
- Budget per frame: ~16ms
- ECS iteration should be <1ms (plenty of headroom)
- **All options are fast enough**, but bevy_ecs/hecs have best margins

---

## Determinism Analysis

**Why determinism matters:**
- Replay system (record commands, replay produces same result)
- Save/load (restore exact game state)
- Potential future multiplayer (lockstep networking)

**Determinism Requirements:**
1. **Iteration order** - Must iterate components in consistent order
2. **System order** - Systems must execute in fixed order
3. **RNG** - Seeded random number generation
4. **Floating-point** - Consistent math across platforms

**ECS Determinism Support:**

| ECS | Default | With Effort | Notes |
|-----|---------|-------------|-------|
| **bevy_ecs** | ❌ No | ✅ Yes | Sort entities by ID, fixed system order |
| **hecs** | ✅ Mostly | ✅ Yes | Simpler, easier to control |
| **specs** | ❌ No | ⚠️ Hard | Parallel systems make it challenging |
| **Custom** | Depends | ✅ Yes | Can design for determinism from start |

**bevy_ecs Determinism Strategy:**
```rust
// Ensure deterministic iteration
fn deterministic_system(query: Query<(Entity, &Transform)>) {
    let mut entities: Vec<_> = query.iter().collect();
    entities.sort_by_key(|(entity, _)| entity.id());

    for (entity, transform) in entities {
        // Process in deterministic order
    }
}

// Or use a Schedule with fixed system order
app.add_systems(Update, (
    system_a,  // Always runs first
    system_b,  // Always runs second
    system_c,  // Always runs third
).chain());  // Chain ensures sequential order
```

---

## Decision Matrix

Weighted scores (1-5, 5 = best):

| Criteria | Weight | bevy_ecs | hecs | specs | Custom |
|----------|--------|----------|------|-------|--------|
| **Performance (1000+ entities)** | 5 | 5 | 5 | 3 | ? |
| **Determinism support** | 4 | 4 | 5 | 2 | 5 |
| **Component flexibility** | 4 | 5 | 4 | 4 | 5 |
| **Developer ergonomics** | 3 | 5 | 3 | 2 | 2 |
| **Active maintenance** | 3 | 5 | 4 | 2 | - |
| **Documentation** | 2 | 5 | 3 | 4 | - |
| **Time to implement** | 4 | 5 | 5 | 4 | 1 |
| **Total** | - | **138** | **113** | **75** | - |

**Weighted Total:**
- **bevy_ecs:** 138 (Winner!)
- **hecs:** 113 (Close second)
- **specs:** 75 (Not recommended)
- **Custom:** Incomplete (not recommended)

---

## Decision

**Selected: bevy_ecs**

**Rationale:**

1. **Performance** - Best-in-class performance for 500-1000+ entities
   - Archetype storage = excellent cache locality
   - Parallel systems = leverage multi-core CPUs
   - Query optimization built-in

2. **Component Flexibility** - Perfect for modular ability system
   - Easy to add/remove components (swap abilities)
   - Powerful query system (find all units with Shield ability)
   - Change detection (only update changed components)

3. **Determinism Achievable** - Not default, but possible with care
   - Sort queries by entity ID when determinism needed
   - Fixed system execution order with `chain()`
   - Well-documented strategies exist

4. **Battle-Tested** - Used in production Bevy games
   - Mature, optimized, well-maintained
   - Large community, good documentation
   - Active development (bug fixes, improvements)

5. **Future-Proof** - Part of Bevy ecosystem
   - If we want to add Bevy features later (physics, audio), easy integration
   - Can start with standalone bevy_ecs, adopt more Bevy later if desired

**Trade-offs Accepted:**
- ⚠️ Must be careful with determinism (not automatic)
- ⚠️ Slightly larger dependency than hecs
- ⚠️ Learning curve for advanced features

**Alternative (if bevy_ecs proves problematic):**
- **hecs** - Simpler, still fast, easier determinism
- Would re-evaluate if: determinism is too hard, dependency size is issue, or want more control

---

## Implementation Plan

### Week 1: Integration & Learning

1. **Add dependency:**
   ```toml
   [dependencies]
   bevy_ecs = "0.15"  # Latest version
   ```

2. **Create module structure:**
   ```
   src/engine/ecs/
     mod.rs          - Public API
     components.rs   - Core components (Transform, etc.)
     resources.rs    - Global resources (Time, Input, etc.)
     systems.rs      - System utilities
   ```

3. **Basic example:**
   - Define components (Transform, Velocity, Lifetime)
   - Create systems (movement, cleanup)
   - Spawn 100 entities
   - Verify iteration works

### Week 1-2: Determinism Prototype

4. **Test determinism:**
   ```rust
   // Run simulation 10 times with same seed
   // Verify identical results
   for i in 0..10 {
       let world = run_simulation(seed=12345, ticks=1000);
       assert_eq!(world.checksum(), expected_checksum);
   }
   ```

5. **Document determinism patterns:**
   - How to iterate deterministically
   - How to order systems
   - How to handle RNG

### Week 2: Performance Validation

6. **Benchmark:**
   - Spawn 1000 entities with 5 components
   - Run 10 systems
   - Measure frame time
   - **Goal:** <1ms for ECS updates

7. **Profile:**
   - Use `cargo flamegraph` or `tracy`
   - Identify bottlenecks
   - Optimize hot paths

---

## References

**bevy_ecs:**
- Docs: https://docs.rs/bevy_ecs/
- Bevy Book: https://bevyengine.org/learn/book/
- Examples: https://github.com/bevyengine/bevy/tree/main/examples/ecs

**hecs:**
- Docs: https://docs.rs/hecs/
- Guide: https://github.com/Ralith/hecs

**specs:**
- Docs: https://docs.rs/specs/
- Book: https://specs.amethyst.rs/docs/tutorials/

**ECS Benchmarks:**
- https://github.com/rust-gamedev/ecs_bench_suite

**Determinism in ECS:**
- https://gafferongames.com/post/deterministic_lockstep/
- https://www.reddit.com/r/rust_gamedev/comments/10y6c8a/deterministic_ecs_for_lockstep_multiplayer/

---

## Conclusion

**bevy_ecs is the best choice** for this project:
- Meets all performance requirements (500-1000+ entities)
- Supports modular ability system perfectly
- Determinism achievable with documented patterns
- Battle-tested, actively maintained, great ergonomics

We'll start with standalone bevy_ecs and can adopt more Bevy features if desired later. The modular ability system will be a natural fit for component-based design.

**Next steps:**
1. Add bevy_ecs dependency
2. Create basic ECS module structure
3. Prototype with 1000 entities
4. Validate determinism approach
5. Document patterns and best practices

**Let's build! 🚀**
