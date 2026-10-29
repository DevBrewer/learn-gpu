use wgpu::Operations;

use crate::{create_render_pipeline, texture};

/// Owns the intermediate HDR render target texture and manages the tonemapping pass.
///
/// In High Dynamic Range (HDR) rendering, scene geometry and lighting are drawn
/// first to an unclipped floating-point intermediate texture (`Rgba16Float`).
/// This pipeline then samples that HDR buffer and runs a tonemapping shader (`hdr.wgsl`)
/// to map high dynamic range colors into the displayable LDR range (`Bgra8UnormSrgb`).
pub struct HdrPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    texture: texture::Texture,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
}

impl HdrPipeline {
    /// Creates a new `HdrPipeline`, initializing the intermediate `Rgba16Float` render target,
    /// its sampling bind group, and the fullscreen tonemapping render pipeline.
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let width = config.width;
        let height = config.height;

        // Use 16-bit float per channel for HDR rendering.
        // `Rgba32Float` is also an option, but often requires extra optional features
        // (such as `FLOAT32_FILTERABLE`) on certain GPU backends.
        let format = wgpu::TextureFormat::Rgba16Float;

        // 1. Create the offscreen HDR render target texture
        let texture = texture::Texture::create_2d_texture(
            device,
            width,
            height,
            format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            wgpu::FilterMode::Nearest,
            Some("Hdr::texture"),
        );

        // 2. Bind group layout for sampling the HDR intermediate texture in the tonemapping shader
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Hdr::layout"),
            entries: &[
                // Binding 0: 2D HDR texture view
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Binding 1: Sampler for reading the HDR texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // 3. Bind group referencing the offscreen HDR texture view and its sampler
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hdr::bind_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        });

        // 4. Create the tonemapping post-processing render pipeline
        let shader = wgpu::include_wgsl!("hdr.wgsl");
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        // Target surface format with sRGB suffix enabled for proper gamma output
        let pipeline = create_render_pipeline(
            device,
            &pipeline_layout,
            config.format.add_srgb_suffix(),
            None,
            &[],
            wgpu::PrimitiveTopology::TriangleList,
            shader,
        );

        Self {
            pipeline,
            bind_group,
            layout,
            texture,
            width,
            height,
            format,
        }
    }

    /// Recreates the intermediate HDR texture and bind group when the window is resized.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.texture = texture::Texture::create_2d_texture(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            wgpu::FilterMode::Nearest,
            Some("Hdr::texture"),
        );
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hdr::bind_group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.texture.sampler),
                },
            ],
        });
        self.width = width;
        self.height = height;
    }

    /// Returns a reference to the offscreen HDR `TextureView` for scene rendering passes.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.texture.view
    }

    /// Returns the color format of the intermediate HDR texture (`wgpu::TextureFormat::Rgba16Float`).
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Executes the fullscreen tonemapping pass, rendering from the internal HDR texture
    /// into the provided target `output` view (typically the surface swapchain texture view).
    pub fn process(&self, encoder: &mut wgpu::CommandEncoder, output: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Hdr::process"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                ops: Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        // Draw a fullscreen triangle (vertices generated procedurally in `hdr.wgsl`)
        pass.draw(0..3, 0..1);
    }
}
