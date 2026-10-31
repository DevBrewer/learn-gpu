use std::collections::HashSet;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, MouseScrollDelta},
    keyboard::KeyCode,
};

/// High-level input state tracker for keyboard and mouse interactions.
#[derive(Debug, Default, Clone)]
pub struct InputState {
    pressed_keys: HashSet<KeyCode>,
    pressed_mouse_buttons: HashSet<MouseButton>,
    cursor_position: (f32, f32),
    cursor_delta: (f32, f32),
    scroll_delta: (f32, f32),
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset per-frame deltas (call at the end of each frame).
    pub fn end_frame(&mut self) {
        self.cursor_delta = (0.0, 0.0);
        self.scroll_delta = (0.0, 0.0);
    }

    /// Check if a keyboard key is currently held down.
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }

    /// Check if a mouse button is currently held down.
    pub fn is_mouse_down(&self, button: MouseButton) -> bool {
        self.pressed_mouse_buttons.contains(&button)
    }

    /// Check if left mouse button is held down.
    pub fn is_left_mouse_down(&self) -> bool {
        self.is_mouse_down(MouseButton::Left)
    }

    /// Check if right mouse button is held down.
    pub fn is_right_mouse_down(&self) -> bool {
        self.is_mouse_down(MouseButton::Right)
    }

    /// Current cursor position in window coordinates (x, y).
    pub fn cursor_pos(&self) -> (f32, f32) {
        self.cursor_position
    }

    /// Cursor movement delta since the last frame (dx, dy).
    pub fn cursor_delta(&self) -> (f32, f32) {
        self.cursor_delta
    }

    /// Mouse scroll wheel delta this frame (dx, dy).
    pub fn scroll_delta(&self) -> (f32, f32) {
        self.scroll_delta
    }

    // Internal update handlers called by the runner:
    pub(crate) fn handle_key(&mut self, key: KeyCode, state: ElementState) {
        match state {
            ElementState::Pressed => {
                self.pressed_keys.insert(key);
            }
            ElementState::Released => {
                self.pressed_keys.remove(&key);
            }
        }
    }

    pub(crate) fn handle_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        match state {
            ElementState::Pressed => {
                self.pressed_mouse_buttons.insert(button);
            }
            ElementState::Released => {
                self.pressed_mouse_buttons.remove(&button);
            }
        }
    }

    pub(crate) fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        let new_x = position.x as f32;
        let new_y = position.y as f32;
        self.cursor_delta.0 += new_x - self.cursor_position.0;
        self.cursor_delta.1 += new_y - self.cursor_position.1;
        self.cursor_position = (new_x, new_y);
    }

    pub(crate) fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        match delta {
            MouseScrollDelta::LineDelta(x, y) => {
                self.scroll_delta.0 += x;
                self.scroll_delta.1 += y;
            }
            MouseScrollDelta::PixelDelta(pos) => {
                self.scroll_delta.0 += pos.x as f32 * 0.05;
                self.scroll_delta.1 += pos.y as f32 * 0.05;
            }
        }
    }
}
