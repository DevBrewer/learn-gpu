//! # pgt: Programmable GPU & UI Toolkit
//!
//! A modern, component-based graphics framework providing high-level building blocks
//! for real-time 3D rendering, shader pipelines, camera control, and GUI integration.

pub mod buffer;
pub mod camera;
pub mod core;
pub mod model;
pub mod pipeline;
pub mod texture;

#[cfg(feature = "hdr")]
pub mod postprocess;

#[cfg(feature = "egui")]
pub mod ui;

/// Convenient prelude re-exporting the most frequently used types and traits.
pub mod prelude {
    pub use crate::buffer::{StorageBinding, UniformBinding};
    pub use crate::camera::{CameraUniform, FlyCamera, OrbitCamera, Projection};
    pub use crate::core::{run, App, Context, InputState};
    pub use crate::model::{DrawModel, Material, Mesh, Model, ModelVertex};
    #[cfg(feature = "gltf")]
    pub use crate::model::load_gltf;
    pub use crate::pipeline::{DepthStencilPreset, PipelineBuilder};
    #[cfg(feature = "hdr")]
    pub use crate::postprocess::HdrPass;
    pub use crate::texture::{DepthTexture, Samplers, Texture2D};

    #[cfg(feature = "egui")]
    pub use crate::ui::EguiPass;

    pub use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
}

pub use core::{run, App, Context, InputState};
