use framework::App;
use rand::RngExt;
use wgpu::util::DeviceExt;
use winit::{event_loop::ActiveEventLoop, keyboard::KeyCode};

// --- Vertex Data Structures ---

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    per_vertex_color: [u8; 4], // unorm8x4 maps normalized RGBA bytes [0..255] -> [0.0..1.0]
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position: vec2<f32>
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                // per_vertex_color: vec4<f32> decoded from 4 normalized bytes
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                },
            ],
        }
    }
}

// Static per-instance data: color and position offset
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct StaticInstance {
    color: [u8; 4],
    offset: [f32; 2],
}

impl StaticInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<StaticInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // color: vec4<f32> (Unorm8x4)
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 0,
                    shader_location: 2,
                },
                // offset: vec2<f32> (Float32x2)
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: std::mem::size_of::<[u8; 4]>() as wgpu::BufferAddress,
                    shader_location: 3,
                },
            ],
        }
    }
}

// Dynamic/changing per-instance data: aspect-ratio-corrected scale
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ChangingInstance {
    scale: [f32; 2],
}

impl ChangingInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ChangingInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // scale: vec2<f32>
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 4,
                },
            ],
        }
    }
}

// --- Donut / Circle Geometry Generator ---

struct CircleData {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

/// Generates a 2D ring (donut) mesh made of alternating inner and outer vertices forming triangle strips.
fn create_circle_vertices(
    radius: f32,
    num_subdivisions: usize,
    inner_radius: f32,
    start_angle: f32,
    end_angle: f32,
) -> CircleData {
    let num_vertices = (num_subdivisions + 1) * 2;
    let mut vertices = Vec::with_capacity(num_vertices);

    // Inner vertices are white (full brightness), outer vertices are dark gray
    let inner_color = [255, 255, 255, 255]; // White
    let outer_color = [25, 25, 25, 255]; // Dark gray (~0.1 * 255)

    for i in 0..=num_subdivisions {
        let angle =
            start_angle + (i as f32) * (end_angle - start_angle) / (num_subdivisions as f32);
        let c = angle.cos();
        let s = angle.sin();

        // Outer rim vertex
        vertices.push(Vertex {
            position: [c * radius, s * radius],
            per_vertex_color: outer_color,
        });

        // Inner rim vertex
        vertices.push(Vertex {
            position: [c * inner_radius, s * inner_radius],
            per_vertex_color: inner_color,
        });
    }

    // Connect quads between adjacent divisions with two triangles each
    let mut indices = Vec::with_capacity(num_subdivisions * 6);
    for i in 0..num_subdivisions {
        let ndx_offset = (i * 2) as u32;

        // First triangle
        indices.push(ndx_offset);
        indices.push(ndx_offset + 1);
        indices.push(ndx_offset + 2);

        // Second triangle
        indices.push(ndx_offset + 2);
        indices.push(ndx_offset + 1);
        indices.push(ndx_offset + 3);
    }

    CircleData { vertices, indices }
}

const NUM_OBJECTS: usize = 100;

// --- State Application Struct ---

struct State {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    static_vertex_buffer: wgpu::Buffer,
    changing_vertex_buffer: wgpu::Buffer,
    base_scales: Vec<f32>,
}

impl App for State {
    fn title() -> &'static str {
        "Donuts Vertex Instances"
    }

    fn init(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
    ) -> anyhow::Result<Self> {
        // 1. Generate circle / donut geometry
        let circle = create_circle_vertices(0.5, 24, 0.25, 0.0, std::f32::consts::PI * 2.0);
        let num_indices = circle.indices.len() as u32;

        // 2. Create vertex & index buffers
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Donut Vertex Buffer"),
            contents: bytemuck::cast_slice(&circle.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Donut Index Buffer"),
            contents: bytemuck::cast_slice(&circle.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // 3. Generate random instance data
        let mut rng = rand::rng();
        let mut static_instances = Vec::with_capacity(NUM_OBJECTS);
        let mut base_scales = Vec::with_capacity(NUM_OBJECTS);

        for _ in 0..NUM_OBJECTS {
            static_instances.push(StaticInstance {
                color: [
                    rng.random_range(0..=255),
                    rng.random_range(0..=255),
                    rng.random_range(0..=255),
                    255,
                ],
                offset: [rng.random_range(-0.9..0.9), rng.random_range(-0.9..0.9)],
            });
            base_scales.push(rng.random_range(0.2..0.5));
        }

        // 4. Create static instance buffer
        let static_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Static Instance Buffer"),
            contents: bytemuck::cast_slice(&static_instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // 5. Create dynamic instance buffer (size for NUM_OBJECTS instances)
        let changing_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Changing Instance Buffer"),
            size: (std::mem::size_of::<ChangingInstance>() * NUM_OBJECTS) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Populate initial changing buffer with current aspect ratio
        let aspect = config.width as f32 / config.height as f32;
        let changing_instances: Vec<ChangingInstance> = base_scales
            .iter()
            .map(|&scale| ChangingInstance {
                scale: [scale / aspect, scale],
            })
            .collect();

        queue.write_buffer(
            &changing_vertex_buffer,
            0,
            bytemuck::cast_slice(&changing_instances),
        );

        // 6. Shader Module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Donuts Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./shader.wgsl").into()),
        });

        // 7. Pipeline Layout
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });

        // 8. Render Pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Donuts Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                // Three vertex buffer slots: [0] mesh geometry, [1] static instances, [2] changing instances
                buffers: &[
                    Some(Vertex::desc()),
                    Some(StaticInstance::desc()),
                    Some(ChangingInstance::desc()),
                ],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            static_vertex_buffer,
            changing_vertex_buffer,
            base_scales,
        })
    }

    fn resize(&mut self, ctx: &mut framework::Context, width: u32, height: u32) {
        if width > 0 && height > 0 {
            ctx.config.width = width;
            ctx.config.height = height;
            ctx.surface.configure(&ctx.device, &ctx.config);

            // Recompute aspect ratio scales on resize and upload to changing buffer
            let aspect = width as f32 / height as f32;
            let changing_instances: Vec<ChangingInstance> = self
                .base_scales
                .iter()
                .map(|&scale| ChangingInstance {
                    scale: [scale / aspect, scale],
                })
                .collect();

            ctx.queue.write_buffer(
                &self.changing_vertex_buffer,
                0,
                bytemuck::cast_slice(&changing_instances),
            );
        }
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        if code == KeyCode::Escape && is_pressed {
            event_loop.exit();
        }
    }

    fn render(&mut self, ctx: &framework::Context, view: &wgpu::TextureView) -> anyhow::Result<()> {
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Donuts Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Donuts Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.3,
                            g: 0.3,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            // Slot 0: geometry vertex buffer
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            // Slot 1: static instance buffer (color & offset)
            render_pass.set_vertex_buffer(1, self.static_vertex_buffer.slice(..));
            // Slot 2: dynamic instance buffer (aspect ratio corrected scale)
            render_pass.set_vertex_buffer(2, self.changing_vertex_buffer.slice(..));
            // Index buffer
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            // Draw NUM_OBJECTS instances of donut mesh
            render_pass.draw_indexed(0..self.num_indices, 0, 0..NUM_OBJECTS as u32);
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    framework::run::<State>()
}
