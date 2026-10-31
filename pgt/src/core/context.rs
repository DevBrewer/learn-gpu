use std::sync::Arc;
use winit::window::Window;
use super::input::InputState;

/// Shared graphics execution context provided to applications.
/// Fields are ordered so that on drop: GPU queues and devices drop first,
/// followed by surface, instance, and lastly window to prevent driver segfaults.
pub struct Context {
    pub config: wgpu::SurfaceConfiguration,
    pub input: InputState,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,
    pub surface: wgpu::Surface<'static>,
    pub instance: wgpu::Instance,
    pub window: Arc<Window>,
}

impl Context {
    /// Surface width in physical pixels.
    pub fn width(&self) -> u32 {
        self.config.width
    }

    /// Surface height in physical pixels.
    pub fn height(&self) -> u32 {
        self.config.height
    }

    /// Aspect ratio (width / height).
    pub fn aspect_ratio(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    /// Surface texture format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Reconfigure the surface on resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}
