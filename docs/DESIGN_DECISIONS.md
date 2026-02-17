# Design Decisions - Locked In

**Date:** 2026-02-17
**Status:** Foundation decisions established

These are the core design decisions that affect technical architecture. They're locked in to prevent scope creep and enable informed technical choices.

---

## Performance & Scale

### Unit Count
**Decision:** 300-500 units baseline, pushing toward **1000+ units**

**Rationale:**
- Matches StarCraft 2 scale (ambitious but achievable)
- Enables large-scale battles
- Differentiates from simpler RTS games

**Technical Implications:**
- MUST use high-performance ECS (bevy_ecs or highly optimized alternative)
- Instanced rendering required (batch similar units)
- LOD system critical (reduce detail for distant units)
- Efficient pathfinding (A* won't scale, need flowfields or optimized approach)
- Spatial partitioning essential (grid or quadtree for collision/queries)

**Success Criteria:**
- 500 units at stable 60 FPS
- 1000 units at 30-60 FPS (stretch goal)

---

### Map Size
**Decision:** ~128x128 tiles (medium-sized maps)

**Note:** Exact tile size TBD based on unit scale during prototyping

**Rationale:**
- Medium maps offer strategic variety
- Not too small (boring) or too large (overwhelming)
- Good balance for pathfinding performance

**Technical Implications:**
- Map = 16,384 tiles (manageable)
- Spatial grid for collision: 128x128 cells or subdivided
- Pathfinding cache size: reasonable
- Memory footprint: ~1-2MB for navigation grid

---

## Art Style & Aesthetics

### Visual Style
**Decision:** Planetary Annihilation detail level + slightly more organic aesthetic

**Characteristics:**
- Clean geometric base shapes
- 2-3 subdivision levels (smooth curves without excessive polys)
- Angular silhouettes with rounded edges
- Minimalist but recognizable

**Reference:**
- **Like:** Planetary Annihilation (geometric clarity)
- **But:** Slightly more organic/curved (not pure hard-surface)

**Technical Implications:**
- Subdivision level: 2-3 (good balance)
- Poly budget per unit: ~2,000-5,000 triangles at max LOD
- With 500 units: ~1-2.5M triangles worst case (manageable with instancing)
- LOD essential: Level 0 (near) = 3 subdivisions, Level 1 (medium) = 2, Level 2 (far) = 1, Level 3 (distant) = 0

**Procedural Modeling Requirements:**
- Skin modifier must create smooth tubes/volumes
- Subdivision must support branching geometry
- Vertex graph complexity: 10-50 vertices per unit

---

## Gameplay Systems

### Ability System
**Decision:** Modular, component-based abilities

**Structure:**
- Each unit has: **1 attack + 1 secondary ability**
- Abilities are **swappable** (pre-game customization)
- Larger units may have additional ability slots (future)

**Example:**
```
Tank Chassis + Laser Attack + Shield Ability = Defensive Tank
Tank Chassis + Plasma Attack + Speed Boost = Raider Tank
```

**Rationale:**
- Player agency and customization
- Replayability (try different loadouts)
- Strategic depth (counter-builds, adaptation)
- Perfect fit for ECS (abilities = components)

**Technical Implications:**
- Component design:
  - `Chassis` component (base stats, model, size)
  - `PrimaryAbility` component (attack ability)
  - `SecondaryAbility` component (utility/special ability)
- Ability as data (registry of available abilities)
- Pre-game loadout UI (select and equip abilities)
- Save/load loadout configurations

**Ability Categories:**
- **Attacks:** Direct damage, AoE, DoT, projectile types
- **Utility:** Shield, speed boost, cloak, heal, repair
- **Control:** Stun, slow, knockback, pull

**Constraints:**
- Not all abilities fit all chassis (e.g., can't put artillery attack on scout)
- Balancing required (some combos might be OP)

---

## Deferred Decisions

These can be decided during implementation:

### Fog of War
**Options:**
- Full fog (unexplored = black, explored = grayed)
- Partial fog (unexplored visible, units hidden)
- No fog (full visibility)

**When to decide:** During map/vision system implementation (Phase 4-5)

---

### Terrain Type
**Options:**
- Flat with height levels (SC2 style)
- Smooth heightmap (Total Annihilation)
- Pure flat (simplest)

**When to decide:** During map system implementation (Phase 4)

**Likely choice:** Flat or height levels (simpler pathfinding)

---

### Win Conditions
**Options:**
- Destroy all enemy buildings (classic)
- Objective-based (capture, escort, defend)
- Hybrid (multiple objectives)

**When to decide:** During campaign/mission system (Phase 7)

**Likely choice:** Hybrid (different objectives per mission)

---

## Anti-Decisions (Scope Boundaries)

What we're **NOT** doing (at least initially):

❌ **Multiplayer** - Single-player only, co-op is future consideration
❌ **Multiple factions** - One faction initially, variety through abilities
❌ **Cover system** - Not a tactical shooter, pure RTS
❌ **Hero units** - Standard RTS units (though "champion" units possible later)
❌ **Base building placement** - Grid-based placement (not free-form like Supreme Commander)

---

## Summary Table

| Aspect | Decision | Impact |
|--------|----------|--------|
| **Unit Count** | 500-1000+ | High-performance ECS required |
| **Map Size** | 128x128 tiles | Medium spatial partitioning |
| **Art Style** | PA + organic (2-3 subdivisions) | Moderate poly budget, LOD critical |
| **Abilities** | Modular (1 attack + 1 secondary) | Component-based, pre-game loadout |
| **Pathfinding** | TBD (A* or flowfield) | Based on performance testing |
| **Terrain** | TBD (likely flat or levels) | Deferred to Phase 4 |
| **Fog of War** | TBD | Deferred to Phase 4-5 |

---

## Next Actions

1. ✅ Design decisions documented
2. ⬜ Research ECS with these constraints
3. ⬜ Prototype ECS with 1000 entities test
4. ⬜ Validate performance claims
5. ⬜ Begin implementation

---

**These decisions are locked in to prevent analysis paralysis. If we discover issues during implementation, we'll document the new findings and adjust course with full context.**
