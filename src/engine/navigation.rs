// Flowfield pathfinding for RTS group movement.
// See docs/research/pathfinding.md for architecture decisions.
//
// Layer 1: NavigationGrid — static walkability per tile.
// Layer 2: FlowField — Dijkstra integration field + gradient directions.
//          Sprint 3: density-feedback surcharge spreads units across corridors.
// Layer 2a: DensityMap — per-cell unit count, rebuilt every N frames,
//           fed into the flowfield cost function as a surcharge.

use glam::{Vec2, Vec3, UVec2};
use std::collections::{BinaryHeap, VecDeque};
use std::cmp::Reverse;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Number of grid cells along X.
pub const GRID_WIDTH: u32 = 100;
/// Number of grid cells along Z.
pub const GRID_HEIGHT: u32 = 100;
/// World units per cell. Cell (cx, cz) covers [cx - WORLD_HALF .. cx+1 - WORLD_HALF].
pub const CELL_SIZE: f32 = 1.0;
/// World runs from -WORLD_HALF to +WORLD_HALF on both X and Z.
pub const WORLD_HALF: f32 = 50.0;

// ============================================================================
// NAVIGATION GRID
// ============================================================================

/// Static tile walkability. Rebuilt when buildings are placed or destroyed.
/// Dynamic obstacles (units) are handled by ORCA (Layer 3), not baked here.
pub struct NavigationGrid {
    /// True if units can walk through this cell.
    pub walkable: Vec<bool>,
    pub width: u32,
    pub height: u32,
}

impl NavigationGrid {
    /// Fully open grid — all cells walkable. Used until terrain is introduced.
    pub fn new_open(width: u32, height: u32) -> Self {
        Self {
            walkable: vec![true; (width * height) as usize],
            width,
            height,
        }
    }

    /// Convert a world-space position (XZ plane) to the grid cell that contains it.
    /// Returns `None` if the position is outside the grid.
    pub fn world_to_cell(&self, pos: Vec3) -> Option<UVec2> {
        let x = (pos.x + WORLD_HALF) / CELL_SIZE;
        let z = (pos.z + WORLD_HALF) / CELL_SIZE;
        if x < 0.0 || z < 0.0 {
            return None;
        }
        let cx = x as u32;
        let cz = z as u32;
        if cx >= self.width || cz >= self.height {
            return None;
        }
        Some(UVec2::new(cx, cz))
    }

    /// Like `world_to_cell` but clamps to grid bounds instead of returning None.
    pub fn world_to_cell_clamped(&self, pos: Vec3) -> UVec2 {
        let cx = ((pos.x + WORLD_HALF) / CELL_SIZE).max(0.0) as u32;
        let cz = ((pos.z + WORLD_HALF) / CELL_SIZE).max(0.0) as u32;
        UVec2::new(cx.min(self.width - 1), cz.min(self.height - 1))
    }

    /// World-space center of a grid cell (y=0, on the ground plane).
    pub fn cell_center(&self, cell: UVec2) -> Vec3 {
        Vec3::new(
            cell.x as f32 * CELL_SIZE - WORLD_HALF + CELL_SIZE * 0.5,
            0.0,
            cell.y as f32 * CELL_SIZE - WORLD_HALF + CELL_SIZE * 0.5,
        )
    }

    #[inline]
    fn idx(&self, cell: UVec2) -> usize {
        (cell.y * self.width + cell.x) as usize
    }

    pub fn is_walkable(&self, cell: UVec2) -> bool {
        self.walkable[self.idx(cell)]
    }
}

// ============================================================================
// DENSITY MAP
// ============================================================================

/// Per-cell unit count, rebuilt every `DENSITY_UPDATE_INTERVAL` frames.
///
/// Fed into `compute_flowfield_with_density` as a surcharge on the Dijkstra
/// cost function. Crowded cells become more expensive to route through, so the
/// flowfield gradient naturally spreads units across the full corridor width
/// instead of funnelling them into a single-file queue.
///
/// See pathfinding.md §"The Novel Part: Density Feedback Cost".
pub struct DensityMap {
    /// Raw unit count per cell. Cleared and rebuilt each update.
    pub counts: Vec<f32>,
    pub width: u32,
    pub height: u32,
}

impl DensityMap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            counts: vec![0.0; (width * height) as usize],
            width,
            height,
        }
    }

    /// Reset all cell counts to zero.
    pub fn clear(&mut self) {
        self.counts.iter_mut().for_each(|c| *c = 0.0);
    }

    /// Increment the count for the cell containing `world_pos`.
    /// Out-of-bounds positions are silently ignored.
    pub fn add_unit(&mut self, world_pos: Vec3) {
        let x = (world_pos.x + WORLD_HALF) / CELL_SIZE;
        let z = (world_pos.z + WORLD_HALF) / CELL_SIZE;
        if x < 0.0 || z < 0.0 {
            return;
        }
        let cx = x as u32;
        let cz = z as u32;
        if cx >= self.width || cz >= self.height {
            return;
        }
        self.counts[(cz * self.width + cx) as usize] += 1.0;
    }

    /// Raw unit count for a grid cell (0.0 if out of bounds).
    #[inline]
    pub fn get(&self, cell: UVec2) -> f32 {
        self.counts
            .get((cell.y * self.width + cell.x) as usize)
            .copied()
            .unwrap_or(0.0)
    }
}

// ============================================================================
// FLOW FIELD
// ============================================================================

/// Pre-computed per-cell movement directions toward a single goal.
///
/// Built once per group order. All units in the group read from this
/// to get their desired velocity, making the per-unit query cost O(1).
pub struct FlowField {
    /// Normalized XZ direction each cell should move in to reach the goal.
    /// Vec2::ZERO means "at goal" or "unreachable".
    pub directions: Vec<Vec2>,

    /// Dijkstra integration cost to goal. `f32::MAX` = unreachable, `0.0` = goal.
    /// With density feedback: effective_cost = terrain_cost + density_weight * density.
    pub integration: Vec<f32>,

    pub width: u32,
    pub height: u32,
    pub goal_cell: UVec2,
}

impl FlowField {
    /// Sample the flow direction for a given grid cell.
    #[inline]
    pub fn sample_cell(&self, cell: UVec2) -> Vec2 {
        let idx = (cell.y * self.width + cell.x) as usize;
        self.directions.get(idx).copied().unwrap_or(Vec2::ZERO)
    }

    /// True if the cell is the goal or within roughly one step of it.
    /// Based on integration cost: goal = 0.0, one normal step ≈ 1.0.
    #[inline]
    pub fn near_goal(&self, cell: UVec2) -> bool {
        let idx = (cell.y * self.width + cell.x) as usize;
        self.integration.get(idx).copied().unwrap_or(f32::MAX) < 2.0
    }
}

// ============================================================================
// FLOWFIELD COMPUTATION — uniform cost (no density)
// ============================================================================

/// Compute a flowfield for the given goal world position using uniform-cost BFS.
///
/// All walkable cells have cost 1.0. Use `compute_flowfield_with_density` for
/// the Sprint 3 density-feedback variant that spreads units across corridors.
pub fn compute_flowfield(grid: &NavigationGrid, goal_world: Vec3) -> FlowField {
    let goal_cell = grid.world_to_cell_clamped(goal_world);
    let size = (grid.width * grid.height) as usize;
    let mut integration = vec![f32::MAX; size];
    let mut queue = VecDeque::new();

    let goal_idx = (goal_cell.y * grid.width + goal_cell.x) as usize;
    if grid.walkable[goal_idx] {
        integration[goal_idx] = 0.0;
        queue.push_back(goal_cell);
    }

    // BFS: expand outward from goal, recording shortest step-distance.
    while let Some(pos) = queue.pop_front() {
        let pos_cost = integration[(pos.y * grid.width + pos.x) as usize];
        for nb in cardinal_neighbors(pos, grid.width, grid.height) {
            let ni = (nb.y * grid.width + nb.x) as usize;
            if grid.walkable[ni] && integration[ni] == f32::MAX {
                integration[ni] = pos_cost + 1.0;
                queue.push_back(nb);
            }
        }
    }

    build_directions(integration, grid.width, grid.height, goal_cell)
}

// ============================================================================
// FLOWFIELD COMPUTATION — Dijkstra with density surcharge (Sprint 3)
// ============================================================================

/// Compute a flowfield using Dijkstra's algorithm with a per-cell density surcharge.
///
/// ```
/// effective_cost(cell) = terrain_cost(cell) + density_weight * unit_density(cell)
/// ```
///
/// Crowded cells become more expensive, so the flowfield gradient routes units
/// away from congestion and spreads them across all available width — like
/// water finding all downhill paths simultaneously.
///
/// `density_weight` controls aggressiveness. The research doc recommends
/// starting at 0.3–0.5 × terrain_cost (i.e. 0.3–0.5 for our uniform grid).
/// Too high → units take very roundabout routes. Too low → corridor pileups.
///
/// See pathfinding.md §"The Novel Part: Density Feedback Cost".
pub fn compute_flowfield_with_density(
    grid: &NavigationGrid,
    goal_world: Vec3,
    density: &DensityMap,
    density_weight: f32,
) -> FlowField {
    let goal_cell = grid.world_to_cell_clamped(goal_world);
    let size = (grid.width * grid.height) as usize;
    let mut integration = vec![f32::MAX; size];

    // Min-heap: Reverse<(cost_bits, cell_x, cell_y)>
    //
    // Trick: for non-negative finite f32 values, IEEE 754 bit patterns sort
    // identically to the float values when interpreted as u32. This lets us
    // use a standard u32 BinaryHeap (which requires Ord) for f32 costs without
    // an external crate.
    let mut heap: BinaryHeap<Reverse<(u32, u32, u32)>> = BinaryHeap::new();

    let goal_idx = (goal_cell.y * grid.width + goal_cell.x) as usize;
    if grid.walkable[goal_idx] {
        integration[goal_idx] = 0.0;
        heap.push(Reverse((0u32, goal_cell.x, goal_cell.y)));
    }

    while let Some(Reverse((cost_bits, cx, cz))) = heap.pop() {
        let entry_cost = f32::from_bits(cost_bits);
        let pos_cost = integration[(cz * grid.width + cx) as usize];

        // Skip stale entries — a shorter path was already recorded for this cell.
        if entry_cost > pos_cost {
            continue;
        }

        let pos = UVec2::new(cx, cz);
        for nb in cardinal_neighbors(pos, grid.width, grid.height) {
            let ni = (nb.y * grid.width + nb.x) as usize;
            if !grid.walkable[ni] {
                continue;
            }

            // Cost to enter neighbor = terrain_cost + density surcharge.
            // terrain_cost = 1.0 (uniform grid; Sprint 4+ may add terrain types).
            let step_cost = 1.0_f32 + density_weight * density.get(nb);
            let new_cost = pos_cost + step_cost;

            if new_cost < integration[ni] {
                integration[ni] = new_cost;
                heap.push(Reverse((new_cost.to_bits(), nb.x, nb.y)));
            }
        }
    }

    build_directions(integration, grid.width, grid.height, goal_cell)
}

// ============================================================================
// SHARED GRADIENT PASS
// ============================================================================

/// Build direction vectors from a completed integration field.
///
/// For each cell, picks the 8-connected neighbor with the lowest cost and
/// stores the normalised direction toward it. This is shared between the
/// uniform-BFS and Dijkstra variants — the gradient logic is identical.
fn build_directions(
    integration: Vec<f32>,
    width: u32,
    height: u32,
    goal_cell: UVec2,
) -> FlowField {
    let size = (width * height) as usize;
    let mut directions = vec![Vec2::ZERO; size];

    for cz in 0..height {
        for cx in 0..width {
            let idx = (cz * width + cx) as usize;
            let cost = integration[idx];
            if cost == f32::MAX || cost == 0.0 {
                // Unreachable → no direction. At goal → stop.
                continue;
            }

            let pos = UVec2::new(cx, cz);
            let mut best_cost = cost;
            let mut best_dir = Vec2::ZERO;

            for nb in all_neighbors(pos, width, height) {
                let ni = (nb.y * width + nb.x) as usize;
                if integration[ni] < best_cost {
                    best_cost = integration[ni];
                    let dx = nb.x as f32 - cx as f32;
                    let dz = nb.y as f32 - cz as f32;
                    // Pre-normalize: dx,dz ∈ {-1,0,1}², only diagonals need correction.
                    let len = (dx * dx + dz * dz).sqrt();
                    best_dir = Vec2::new(dx / len, dz / len);
                }
            }

            directions[idx] = best_dir;
        }
    }

    FlowField {
        directions,
        integration,
        width,
        height,
        goal_cell,
    }
}

// ============================================================================
// NEIGHBOR ITERATORS
// ============================================================================

/// The four cardinal (N/S/E/W) grid neighbors of a cell, clamped to bounds.
fn cardinal_neighbors(pos: UVec2, w: u32, h: u32) -> impl Iterator<Item = UVec2> {
    let (x, z) = (pos.x as i32, pos.y as i32);
    let (wi, hi) = (w as i32, h as i32);
    [(x - 1, z), (x + 1, z), (x, z - 1), (x, z + 1)]
        .into_iter()
        .filter(move |&(nx, nz)| nx >= 0 && nz >= 0 && nx < wi && nz < hi)
        .map(|(nx, nz)| UVec2::new(nx as u32, nz as u32))
}

/// All eight (cardinal + diagonal) neighbors, clamped to bounds.
/// Used for gradient computation so directions can point diagonally.
fn all_neighbors(pos: UVec2, w: u32, h: u32) -> impl Iterator<Item = UVec2> {
    let (x, z) = (pos.x as i32, pos.y as i32);
    let (wi, hi) = (w as i32, h as i32);
    [
        (-1, -1), (0, -1), (1, -1),
        (-1,  0),          (1,  0),
        (-1,  1), (0,  1), (1,  1),
    ]
    .into_iter()
    .filter(move |&(dx, dz)| {
        let nx = x + dx;
        let nz = z + dz;
        nx >= 0 && nz >= 0 && nx < wi && nz < hi
    })
    .map(move |(dx, dz)| UVec2::new((x + dx) as u32, (z + dz) as u32))
}
