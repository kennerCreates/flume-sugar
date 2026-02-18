// Engine module - reusable game engine components
// See docs/research/ecs-choice.md for ECS architecture decisions

pub mod components;
pub mod debug_overlay;
pub mod systems;

// Re-export commonly used items
pub use components::*;
