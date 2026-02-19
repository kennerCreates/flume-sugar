// Input state tracking for keyboard and mouse
// Abstracts winit events into a queryable per-frame snapshot

use std::collections::HashSet;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct InputState {
    // Keyboard
    keys_held: HashSet<KeyCode>,
    keys_just_pressed: HashSet<KeyCode>,

    // Mouse
    pub mouse_position: (f32, f32),
    /// Accumulated mouse movement this frame. Reset to (0, 0) in end_frame().
    /// Computed incrementally in process_event so it reflects the current frame,
    /// not the previous one.
    pub mouse_delta: (f32, f32),

    // Scroll: accumulated vertical scroll this frame, reset in end_frame()
    pub scroll_delta: f32,

    // Mouse buttons
    pub middle_mouse_held: bool,
    pub right_mouse_held: bool,

    // Window dimensions (used for edge scrolling)
    pub window_size: (u32, u32),
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys_held: HashSet::new(),
            keys_just_pressed: HashSet::new(),
            mouse_position: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
            scroll_delta: 0.0,
            middle_mouse_held: false,
            right_mouse_held: false,
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
                        ElementState::Pressed => {
                            if !self.keys_held.contains(&key) {
                                self.keys_just_pressed.insert(key);
                            }
                            self.keys_held.insert(key);
                        }
                        ElementState::Released => {
                            self.keys_held.remove(&key);
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = (position.x as f32, position.y as f32);
                // Accumulate delta so multiple CursorMoved events in one frame add up correctly
                self.mouse_delta.0 += new_pos.0 - self.mouse_position.0;
                self.mouse_delta.1 += new_pos.1 - self.mouse_position.1;
                self.mouse_position = new_pos;
            }
            WindowEvent::MouseInput { state, button: MouseButton::Middle, .. } => {
                self.middle_mouse_held = *state == ElementState::Pressed;
            }
            WindowEvent::MouseInput { state, button: MouseButton::Right, .. } => {
                self.right_mouse_held = *state == ElementState::Pressed;
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
        self.keys_just_pressed.clear();
        self.scroll_delta = 0.0;
        self.mouse_delta = (0.0, 0.0);
    }

    /// True only during the frame the key was first pressed. Use this for
    /// one-shot actions (toggle menus, select units, etc.) rather than is_key_held.
    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.contains(&key)
    }

}
