// Engine module - reusable game engine components
// See docs/research/ecs-choice.md for ECS architecture decisions
// See docs/research/procedural-modeling.md for mesh/skin/subdivision decisions
// See docs/research/pathfinding.md for navigation/crowd simulation decisions

pub mod camera;
pub mod components;
pub mod debug_overlay;
pub mod input;
pub mod mesh;
pub mod navigation;
pub mod orca;
pub mod skin;
pub mod subdivide;
pub mod systems;

// Re-export commonly used items
pub use components::*;
pub use mesh::triangulate_smooth;
pub use navigation::{NavigationGrid, FlowField, DensityMap, compute_flowfield, compute_flowfield_with_density, GRID_WIDTH, GRID_HEIGHT};
pub use orca::{AgentSnapshot, SpatialGrid, compute_orca_velocity};
pub use skin::{SkinGraph, skin_modifier};
pub use subdivide::subdivide;
