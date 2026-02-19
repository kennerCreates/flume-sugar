// Flowfield pathfinding for RTS group movement.
// See docs/research/pathfinding.md for architecture decisions.
//
// Layer 1: NavigationGrid — static walkability per tile.
// Layer 2: FlowField — BFS integration field + gradient directions.
//
// Sprint 1: uniform-cost BFS, no density feedback yet.
// Sprint 3 will add density surcharge to the cost function.

use glam::{Vec2, Vec3, UVec2};
use std::collections::VecDeque;

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

    /// BFS distance to goal in grid steps. `u32::MAX` = unreachable.
    /// Kept for the density-feedback system (Sprint 3).
    pub integration: Vec<u32>,

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

    /// True if the cell is the goal or within 1 step of it.
    #[inline]
    pub fn near_goal(&self, cell: UVec2) -> bool {
        let idx = (cell.y * self.width + cell.x) as usize;
        self.integration.get(idx).copied().unwrap_or(u32::MAX) <= 1
    }
}

// ============================================================================
// FLOWFIELD COMPUTATION
// ============================================================================

/// Compute a flowfield for the given goal world position.
///
/// Algorithm:
/// 1. BFS (4-connected) from goal outward, building an integration field.
/// 2. Gradient pass: for each cell, find the 8-connected neighbor with the
///    smallest integration value and point toward it.
///
/// Sprint 1: uniform cost (BFS). Sprint 3 will switch to Dijkstra with a
/// density surcharge to spread units across the full corridor width.
pub fn compute_flowfield(grid: &NavigationGrid, goal_world: Vec3) -> FlowField {
    let goal_cell = grid.world_to_cell_clamped(goal_world);
    let size = (grid.width * grid.height) as usize;
    let mut integration = vec![u32::MAX; size];
    let mut queue = VecDeque::new();

    let goal_idx = (goal_cell.y * grid.width + goal_cell.x) as usize;
    if grid.walkable[goal_idx] {
        integration[goal_idx] = 0;
        queue.push_back(goal_cell);
    }

    // BFS: expand outward from goal, recording shortest step-distance.
    while let Some(pos) = queue.pop_front() {
        let pos_cost = integration[(pos.y * grid.width + pos.x) as usize];
        for nb in cardinal_neighbors(pos, grid.width, grid.height) {
            let ni = (nb.y * grid.width + nb.x) as usize;
            if grid.walkable[ni] && integration[ni] == u32::MAX {
                integration[ni] = pos_cost + 1;
                queue.push_back(nb);
            }
        }
    }

    // Gradient pass: for each non-goal cell, find the 8-connected neighbor
    // with the lowest integration value and compute a direction toward it.
    let mut directions = vec![Vec2::ZERO; size];
    for cz in 0..grid.height {
        for cx in 0..grid.width {
            let idx = (cz * grid.width + cx) as usize;
            let cost = integration[idx];
            if cost == u32::MAX || cost == 0 {
                // Unreachable → no direction. At goal → stop.
                continue;
            }

            let pos = UVec2::new(cx, cz);
            let mut best_cost = cost;
            let mut best_dir = Vec2::ZERO;

            for nb in all_neighbors(pos, grid.width, grid.height) {
                let ni = (nb.y * grid.width + nb.x) as usize;
                if integration[ni] < best_cost {
                    best_cost = integration[ni];
                    let dx = nb.x as f32 - cx as f32;
                    let dz = nb.y as f32 - cz as f32;
                    // Pre-normalize (dx,dz is always within {-1,0,1}² so
                    // the only non-unit case is diagonals at length sqrt(2)).
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
        width: grid.width,
        height: grid.height,
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
