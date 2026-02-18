// Input state tracking for keyboard and mouse
// Abstracts winit events into a queryable per-frame snapshot

use std::collections::HashSet;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct InputState {
    // Keyboard
    keys_held: HashSet<KeyCode>,

    // Mouse
    pub mouse_position: (f32, f32),
    mouse_prev_position: (f32, f32),
    pub mouse_delta: (f32, f32),

    // Scroll: accumulated vertical scroll this frame, reset in end_frame()
    pub scroll_delta: f32,

    // Window dimensions (used for edge scrolling)
    pub window_size: (u32, u32),
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys_held: HashSet::new(),
            mouse_position: (0.0, 0.0),
            mouse_prev_position: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
            scroll_delta: 0.0,
            window_size: (0, 0),
        }
    }

    /// Feed a winit WindowEvent into the input state.
    /// Call this once per event before the game's own event handling.
    pub fn process_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => { self.keys_held.insert(key); }
                        ElementState::Released => { self.keys_held.remove(&key); }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                };
                self.scroll_delta += y;
            }
            WindowEvent::Resized(size) => {
                self.window_size = (size.width, size.height);
            }
            _ => {}
        }
    }

    /// Call once per frame after update() and render() have consumed input.
    /// Resets per-frame accumulators.
    pub fn end_frame(&mut self) {
        self.scroll_delta = 0.0;
        self.mouse_delta = (
            self.mouse_position.0 - self.mouse_prev_position.0,
            self.mouse_position.1 - self.mouse_prev_position.1,
        );
        self.mouse_prev_position = self.mouse_position;
    }

    pub fn is_key_held(&self, key: KeyCode) -> bool {
        self.keys_held.contains(&key)
    }
}
