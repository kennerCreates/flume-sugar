// ORCA — Optimal Reciprocal Collision Avoidance (Sprint 2).
//
// Based on: van den Berg, Guy, Lin, Manocha — "Reciprocal n-Body Collision
// Avoidance" (2011).  The LP solver is a direct Rust translation of the
// RVO2 reference implementation (Apache 2.0).
//
// See docs/research/pathfinding.md §"Layer 3: ORCA" for design decisions,
// including the Predictive-ORCA enhancement (neighbors are assumed to move at
// their flowfield desired velocity, not their actual previous-frame velocity).

use glam::Vec2;

const EPSILON: f32 = 1e-5;
/// Maximum number of inter-group neighbours considered per agent per frame.
/// Capping this prevents the LP from becoming infeasible in dense crossings.
const MAX_ORCA_NEIGHBORS: usize = 10;

// ============================================================================
// SPATIAL GRID
// ============================================================================

/// Uniform spatial hash grid for O(1) neighbour lookup.
///
/// Stores indices into the caller's agent-snapshot slice.
/// Cleared and rebuilt every frame before ORCA runs.
pub struct SpatialGrid {
    cells: Vec<Vec<usize>>,
    cell_size: f32,
    width: u32,
    height: u32,
    world_min: Vec2, // bottom-left corner of the grid in world XZ
}

impl SpatialGrid {
    /// `world_min` / `world_max` define the bounds the grid covers (XZ plane).
    /// `cell_size` is the side length of each cell in world units.
    pub fn new(world_min: Vec2, world_max: Vec2, cell_size: f32) -> Self {
        let span = world_max - world_min;
        let width  = (span.x / cell_size).ceil() as u32 + 2;
        let height = (span.y / cell_size).ceil() as u32 + 2;
        Self {
            cells: vec![Vec::new(); (width * height) as usize],
            cell_size,
            width,
            height,
            world_min,
        }
    }

    /// Remove all stored indices.  Call once per frame before inserting.
    pub fn clear(&mut self) {
        for c in &mut self.cells { c.clear(); }
    }

    fn cell_xy(&self, pos: Vec2) -> Option<(u32, u32)> {
        let cx = ((pos.x - self.world_min.x) / self.cell_size) as i32;
        let cy = ((pos.y - self.world_min.y) / self.cell_size) as i32;
        if cx >= 0 && cy >= 0 && (cx as u32) < self.width && (cy as u32) < self.height {
            Some((cx as u32, cy as u32))
        } else {
            None
        }
    }

    /// Insert agent `idx` at world position `pos`.
    pub fn insert(&mut self, pos: Vec2, idx: usize) {
        if let Some((cx, cy)) = self.cell_xy(pos) {
            self.cells[(cy * self.width + cx) as usize].push(idx);
        }
    }

    /// Append to `out` all agent indices in cells within `radius` of `pos`.
    ///
    /// Returns a superset — callers must distance-filter results.
    /// Does not clear `out` before writing.
    pub fn query_radius(&self, pos: Vec2, radius: f32, out: &mut Vec<usize>) {
        let r_cells = (radius / self.cell_size).ceil() as i32 + 1;
        let (cx0, cy0) = match self.cell_xy(pos) {
            Some(p) => (p.0 as i32, p.1 as i32),
            None    => return,
        };
        for dy in -r_cells..=r_cells {
            for dx in -r_cells..=r_cells {
                let cx = cx0 + dx;
                let cy = cy0 + dy;
                if cx < 0 || cy < 0
                    || cx >= self.width  as i32
                    || cy >= self.height as i32
                {
                    continue;
                }
                out.extend_from_slice(
                    &self.cells[(cy as u32 * self.width + cx as u32) as usize],
                );
            }
        }
    }
}

// ============================================================================
// AGENT SNAPSHOT
// ============================================================================

/// Read-only data for one agent, collected from ECS before ORCA runs.
pub struct AgentSnapshot {
    /// XZ position.
    pub pos: Vec2,
    /// XZ velocity from the previous frame (used as the agent's own
    /// "current" velocity when building its ORCA constraints).
    pub vel: Vec2,
    /// Desired velocity = flowfield_direction * max_speed.
    /// Used as the *neighbour's* expected velocity (Predictive-ORCA).
    pub desired_vel: Vec2,
    pub radius: f32,
    pub max_speed: f32,
    /// Group membership.  ORCA constraints are only generated for agents
    /// in *different* groups — same-group units move in formation and do
    /// not need to mutually avoid each other.
    pub group_id: u32,
    /// ORCA priority.  Lower value = higher rank = holds course.
    /// When two agents differ in priority, the higher-priority agent takes
    /// less responsibility (0.2) and the lower-priority agent takes more (0.8).
    /// Equal-priority agents split responsibility 50/50 as standard ORCA.
    pub priority: u32,
}

// ============================================================================
// ORCA HALFPLANE
// ============================================================================

/// A directed halfplane constraint in velocity space.
///
/// Convention (matches RVO2):
///   feasible region = { v : det(dir, point - v) ≤ 0 }
///   i.e. v must lie on the LEFT of the ray from `point` in direction `dir`.
#[derive(Clone, Copy)]
struct OrcaLine {
    point: Vec2,
    dir:   Vec2,
}

/// 2D determinant / cross product: det(a, b) = a.x·b.y − a.y·b.x
#[inline]
fn det(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Compute the ORCA halfplane that agent A must respect to avoid agent B.
///
/// `time_horizon` — look-ahead window for avoidance (seconds).
/// `inv_dt`       — 1 / frame_dt, used only when agents already overlap.
fn orca_halfplane(
    pos_a: Vec2, vel_a: Vec2, r_a: f32,
    pos_b: Vec2, vel_b: Vec2, r_b: f32,
    time_horizon: f32,
    inv_dt:       f32,
    respons_a:    f32,  // fraction of the required velocity change A must take
) -> OrcaLine {
    let rel_pos = pos_b - pos_a;
    let rel_vel = vel_a - vel_b;
    let dist_sq     = rel_pos.length_squared();
    let combined_r  = r_a + r_b;
    let combined_rsq = combined_r * combined_r;

    let u: Vec2;
    let line_dir: Vec2;

    if dist_sq > combined_rsq {
        // ── Agents not overlapping ───────────────────────────────────────────
        // w: relative velocity shifted to the truncated-cone tip.
        let w    = rel_vel - rel_pos / time_horizon;
        let w_sq = w.length_squared();
        let dot  = w.dot(rel_pos);

        if dot < 0.0 && dot * dot > combined_rsq * w_sq {
            // Closest boundary point is on the circular "cap" at t = tau.
            let w_len  = w_sq.sqrt();
            let unit_w = if w_len > EPSILON { w / w_len } else { Vec2::X };
            line_dir = Vec2::new(unit_w.y, -unit_w.x);
            u = (combined_r / time_horizon - w_len) * unit_w;
        } else {
            // Closest boundary point is on one of the cone "legs".
            let leg = (dist_sq - combined_rsq).max(0.0).sqrt();
            // 2D cross product rel_pos × w determines left or right leg.
            if det(rel_pos, w) > 0.0 {
                line_dir = Vec2::new(
                     rel_pos.x * leg - rel_pos.y * combined_r,
                     rel_pos.x * combined_r + rel_pos.y * leg,
                ) / dist_sq;
            } else {
                line_dir = -Vec2::new(
                     rel_pos.x * leg + rel_pos.y * combined_r,
                    -rel_pos.x * combined_r + rel_pos.y * leg,
                ) / dist_sq;
            }
            u = rel_vel.dot(line_dir) * line_dir - rel_vel;
        }
    } else {
        // ── Agents already overlapping — resolve with per-frame horizon ──────
        let w     = rel_vel - rel_pos * inv_dt;
        let w_len = w.length();
        let unit_w = if w_len > EPSILON {
            w / w_len
        } else if rel_pos.length_squared() > EPSILON * EPSILON {
            // Push directly away from neighbour centre.
            -rel_pos.normalize()
        } else {
            Vec2::X
        };
        line_dir = Vec2::new(unit_w.y, -unit_w.x);
        u = (combined_r * inv_dt - w_len) * unit_w;
    }

    OrcaLine {
        point: vel_a + respons_a * u,
        dir:   line_dir,
    }
}

// ============================================================================
// 2-D LINEAR PROGRAMME  (translated from RVO2)
// ============================================================================

/// Solve the 1-D sub-problem for constraint `line_no`, given that constraints
/// 0..line_no-1 are already satisfied.
///
/// `dir_opt = true`  → optimise in the direction of `pref_vel` (used in lp3).
/// `dir_opt = false` → find the closest point to `pref_vel`.
///
/// Returns `false` if the constraint set is infeasible for this sub-problem.
fn lp1(
    lines:   &[OrcaLine],
    line_no: usize,
    max_spd: f32,
    pref_vel: Vec2,
    dir_opt: bool,
    result:  &mut Vec2,
) -> bool {
    let line  = &lines[line_no];
    let dot   = line.point.dot(line.dir);
    let disc  = dot * dot + max_spd * max_spd - line.point.length_squared();
    if disc < 0.0 {
        // Speed circle fully cuts off this halfplane.
        return false;
    }
    let sq = disc.sqrt();
    let mut t_left  = -dot - sq;
    let mut t_right = -dot + sq;

    for i in 0..line_no {
        let denom = det(line.dir, lines[i].dir);
        let numer = det(lines[i].dir, line.point - lines[i].point);
        if denom.abs() <= EPSILON {
            if numer < 0.0 { return false; }
            continue;
        }
        let t = numer / denom;
        if denom < 0.0 {
            t_right = t_right.min(t);
        } else {
            t_left  = t_left.max(t);
        }
        if t_left > t_right { return false; }
    }

    if dir_opt {
        // Pick the extreme t in the direction of pref_vel.
        if line.dir.dot(pref_vel) > 0.0 {
            *result = line.point + t_right * line.dir;
        } else {
            *result = line.point + t_left  * line.dir;
        }
    } else {
        // Closest point on this constraint line to pref_vel.
        let t = line.dir.dot(pref_vel - line.point);
        *result = line.point + t.clamp(t_left, t_right) * line.dir;
    }
    true
}

/// Main 2-D LP: iterate over all halfplane constraints.
///
/// Returns the index of the first constraint that could not be satisfied
/// (= `lines.len()` if all are satisfied).
fn lp2(
    lines:    &[OrcaLine],
    max_spd:  f32,
    pref_vel: Vec2,
    dir_opt:  bool,
    result:   &mut Vec2,
) -> usize {
    if dir_opt {
        *result = pref_vel.normalize_or_zero() * max_spd;
    } else if result.length_squared() > max_spd * max_spd {
        *result = result.normalize() * max_spd;
    }

    for i in 0..lines.len() {
        // Violation: result is on the wrong (right) side of constraint i.
        if det(lines[i].dir, lines[i].point - *result) > 0.0 {
            let prev = *result;
            if !lp1(lines, i, max_spd, pref_vel, dir_opt, result) {
                *result = prev;
                return i;
            }
        }
    }
    lines.len()
}

/// Fallback LP3: when constraints are mutually infeasible, minimise the
/// maximum halfplane violation depth (least-bad velocity).
fn lp3(lines: &[OrcaLine], begin: usize, max_spd: f32, result: &mut Vec2) {
    let mut distance = 0.0f32;
    for i in begin..lines.len() {
        let viol = det(lines[i].dir, lines[i].point - *result);
        if viol > distance {
            // Project all earlier constraints onto the boundary of constraint i.
            let mut proj: Vec<OrcaLine> = Vec::with_capacity(i);
            for j in 0..i {
                let d = det(lines[i].dir, lines[j].dir);
                if d.abs() <= EPSILON {
                    if lines[i].dir.dot(lines[j].dir) > 0.0 {
                        // Same direction: already subsumed.
                        continue;
                    }
                    // Opposite direction: midpoint constraint.
                    proj.push(OrcaLine {
                        point: 0.5 * (lines[i].point + lines[j].point),
                        dir:   (lines[j].dir - lines[i].dir).normalize_or_zero(),
                    });
                } else {
                    let t = det(lines[j].dir, lines[i].point - lines[j].point) / d;
                    proj.push(OrcaLine {
                        point: lines[i].point + t * lines[i].dir,
                        dir:   (lines[j].dir - lines[i].dir).normalize_or_zero(),
                    });
                }
            }
            let opt_dir = Vec2::new(-lines[i].dir.y, lines[i].dir.x);
            let prev    = *result;
            if lp2(&proj, max_spd, opt_dir, true, result) < proj.len() {
                // Numerical edge case — keep previous best.
                *result = prev;
            }
            distance = det(lines[i].dir, lines[i].point - *result);
        }
    }
}

// ============================================================================
// PUBLIC API
// ============================================================================

/// Compute the ORCA-adjusted velocity for agent `a_idx`.
///
/// Reads neighbour data from `agents` via `grid`.  The returned velocity is
/// guaranteed to be within `agents[a_idx].max_speed` and tries to stay as
/// close as possible to `agents[a_idx].desired_vel`.
///
/// **Predictive-ORCA enhancement** (pathfinding.md):
/// Neighbour velocities are taken from `desired_vel` (flowfield direction)
/// rather than actual previous-frame velocities, giving smooth anticipatory
/// lane changes instead of last-moment swerves.
pub fn compute_orca_velocity(
    agents:       &[AgentSnapshot],
    a_idx:        usize,
    grid:         &SpatialGrid,
    time_horizon: f32,
    inv_dt:       f32,
) -> Vec2 {
    let a = &agents[a_idx];

    // Neighbour search radius: combined physical radii + full velocity cone.
    let search_r = a.radius + a.max_speed * time_horizon + 1.0;

    let mut candidates: Vec<usize> = Vec::new();
    grid.query_radius(a.pos, search_r, &mut candidates);
    // Spatial query returns a superset; sort + dedup to avoid double-counting
    // cells at grid boundaries.
    candidates.sort_unstable();
    candidates.dedup();

    // Filter to inter-group neighbours within reach and sort by distance
    // so the neighbour cap keeps the most pressing collisions.
    let reach_sq_base = {
        let reach = a.radius + a.max_speed * time_horizon;
        reach * reach
    };
    let mut inter_group: Vec<(f32, usize)> = candidates
        .iter()
        .filter_map(|&b_idx| {
            if b_idx == a_idx { return None; }
            let b = &agents[b_idx];
            if b.group_id == a.group_id { return None; }
            let reach = a.radius + b.radius + a.max_speed * time_horizon;
            let dist_sq = (b.pos - a.pos).length_squared();
            if dist_sq > reach * reach { return None; }
            Some((dist_sq, b_idx))
        })
        .collect();
    inter_group.sort_unstable_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
    inter_group.truncate(MAX_ORCA_NEIGHBORS);

    let _ = reach_sq_base; // suppress unused warning

    let mut constraints: Vec<OrcaLine> = Vec::with_capacity(inter_group.len());
    for (_, b_idx) in &inter_group {
        let b = &agents[*b_idx];

        // Priority-based responsibility.
        // Lower priority value = higher rank = holds course (takes less responsibility).
        let respons_a = if a.priority == b.priority {
            0.5   // standard symmetric ORCA
        } else if a.priority < b.priority {
            0.2   // a outranks b — a holds course, b will take the larger share
        } else {
            0.8   // b outranks a — a steps aside
        };

        // Predictive: assume neighbour will follow its desired velocity.
        constraints.push(orca_halfplane(
            a.pos, a.vel,         a.radius,
            b.pos, b.desired_vel, b.radius,
            time_horizon,
            inv_dt,
            respons_a,
        ));
    }

    // Solve 2-D LP: closest feasible velocity to desired_vel.
    let mut result = a.desired_vel;
    let fail = lp2(&constraints, a.max_speed, a.desired_vel, false, &mut result);
    if fail < constraints.len() {
        lp3(&constraints, fail, a.max_speed, &mut result);
    }
    result
}
