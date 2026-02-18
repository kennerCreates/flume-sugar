// Engine module - reusable game engine components
// See docs/research/ecs-choice.md for ECS architecture decisions

pub mod camera;
pub mod components;
pub mod debug_overlay;
pub mod input;
pub mod systems;

// Re-export commonly used items
pub use components::*;
