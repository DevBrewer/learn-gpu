use std::{f32::consts::FRAC_PI_2, time::Duration};

use glam::{
    Mat4, Vec3,
    camera::rh::{proj::directx::perspective, view::look_to_mat4},
};
use winit::{dpi::PhysicalPosition, event::MouseScrollDelta, keyboard::KeyCode};

/// A small margin subtracted from PI / 2 (90 degrees) to prevent pitch gimbal lock / flipping.
pub const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;

// ---------------------------------------------------------------------------
// Camera (First-Person / Free-Look)
// ---------------------------------------------------------------------------

/// Represents a first-person perspective camera defined by position, yaw, and pitch.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// World-space position of the camera eye.
    pub position: Vec3,
    /// Horizontal rotation in radians around the world Y-axis.
    pub yaw: f32,
    /// Vertical rotation in radians above/below the horizontal plane.
    pub pitch: f32,
}

impl Camera {
    /// Creates a new camera instance.
    ///
    /// * `position`: Initial world position (converts from anything implementing `Into<Vec3>`).
    /// * `yaw`: Horizontal angle in radians.
    /// * `pitch`: Vertical angle in radians.
    pub fn new<V: Into<Vec3>>(position: V, yaw: f32, pitch: f32) -> Self {
        Self {
            position: position.into(),
            yaw,
            pitch,
        }
    }

    /// Computes the right-handed view matrix for this camera.
    ///
    /// Constructs a direction unit vector from yaw and pitch, then applies
    /// `look_to_mat4` pointing from `position` in that forward direction.
    pub fn calc_matrix(&self) -> Mat4 {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();

        // Unit vector pointing in the direction the camera is facing
        let forward = Vec3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize();

        look_to_mat4(self.position, forward, Vec3::Y)
    }
}

// ---------------------------------------------------------------------------
// Projection Matrix
// ---------------------------------------------------------------------------

/// Perspective projection settings targeting WebGPU depth clip space [0, 1].
#[derive(Debug, Clone, Copy)]
pub struct Projection {
    pub aspect_ratio: f32,
    pub fovy: f32, // in radians
    pub znear: f32,
    pub zfar: f32,
}

impl Projection {
    /// Creates a new projection configuration.
    pub fn new(width: u32, height: u32, fovy: f32, znear: f32, zfar: f32) -> Self {
        Self {
            aspect_ratio: width as f32 / height.max(1) as f32,
            fovy,
            znear,
            zfar,
        }
    }

    /// Updates aspect ratio when the window or surface is resized.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect_ratio = width as f32 / height.max(1) as f32;
    }

    /// Computes the perspective projection matrix for WebGPU (right-handed, Z depth in [0, 1]).
    pub fn calc_matrix(&self) -> Mat4 {
        perspective(self.fovy, self.aspect_ratio, self.znear, self.zfar)
    }
}

// ---------------------------------------------------------------------------
// Camera Controller
// ---------------------------------------------------------------------------

/// Handles user input (keyboard, mouse motion, mouse wheel scroll) and updates camera state.
#[derive(Debug)]
pub struct CameraController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    scroll: f32,
    pub speed: f32,
    pub sensitivity: f32,
}

impl CameraController {
    /// Creates a new controller with given movement speed and mouse sensitivity.
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            scroll: 0.0,
            speed,
            sensitivity,
        }
    }

    /// Processes keyboard events for movement (WASD, Arrow keys, Space for up, Shift for down).
    /// Returns `true` if the key was recognized and handled.
    pub fn handle_key(&mut self, key: KeyCode, pressed: bool) -> bool {
        let amount = if pressed { 1.0 } else { 0.0 };
        match key {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.amount_forward = amount;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.amount_backward = amount;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.amount_right = amount;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.amount_left = amount;
                true
            }
            KeyCode::Space => {
                self.amount_up = amount;
                true
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                self.amount_down = amount;
                true
            }
            _ => false,
        }
    }

    /// Accumulates mouse movement deltas (in screen pixels).
    pub fn handle_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal += mouse_dx as f32;
        self.rotate_vertical += mouse_dy as f32;
    }

    /// Accumulates mouse scroll wheel delta for zoom / dollying along the view axis.
    pub fn handle_scroll(&mut self, delta: &MouseScrollDelta) {
        self.scroll += match delta {
            // Line scroll (e.g. mouse wheel notches, roughly 100px per line in tutorial)
            MouseScrollDelta::LineDelta(_, scroll) => -scroll * 100.0,
            // Pixel scroll (e.g. trackpads)
            MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => -*scroll as f32,
        };
    }

    /// Updates the camera position, yaw, and pitch according to elapsed time `dt`.
    pub fn update_camera(&mut self, camera: &mut Camera, dt: Duration) {
        let dt = dt.as_secs_f32();

        // 1. Move horizontally along camera yaw (XZ plane)
        let (yaw_sin, yaw_cos) = camera.yaw.sin_cos();
        let forward = Vec3::new(yaw_cos, 0.0, yaw_sin).normalize();
        let right = Vec3::new(-yaw_sin, 0.0, yaw_cos).normalize();

        camera.position += forward * (self.amount_forward - self.amount_backward) * self.speed * dt;
        camera.position += right * (self.amount_right - self.amount_left) * self.speed * dt;

        // 2. Move in/out (zoom along full 3D facing vector via mouse scroll)
        let (pitch_sin, pitch_cos) = camera.pitch.sin_cos();
        let scrollward = Vec3::new(pitch_cos * yaw_cos, pitch_sin, pitch_cos * yaw_sin).normalize();
        camera.position += scrollward * self.scroll * self.speed * self.sensitivity * dt;
        self.scroll = 0.0;

        // 3. Move world up/down
        camera.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

        // 4. Rotate yaw & pitch based on mouse delta
        // `rotate_horizontal` turns left/right; `rotate_vertical` pitches up/down.
        // Direct radian scaling using sensitivity and dt:
        camera.yaw += self.rotate_horizontal * self.sensitivity * dt;
        camera.pitch -= self.rotate_vertical * self.sensitivity * dt;

        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        // 5. Clamp pitch to [-SAFE_FRAC_PI_2, SAFE_FRAC_PI_2] to prevent flipping upside down
        camera.pitch = camera.pitch.clamp(-SAFE_FRAC_PI_2, SAFE_FRAC_PI_2);
    }
}
