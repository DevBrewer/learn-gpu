use framework::App;
use glam::{
    Mat3, Mat4, Quat, Vec3,
    camera::rh::{proj::directx::perspective, view::look_at_mat4},
};
use model::{DrawModel, Vertex};
use wgpu::util::DeviceExt;
use winit::{event_loop::ActiveEventLoop, keyboard::KeyCode};

mod model;
mod resources;
mod texture;

// Configuration for instances arrangement
const NUM_INSTANCES_PER_ROW: u32 = 10;

// ---------------------------------------------------------------------------
// Camera implementation using glam
// ---------------------------------------------------------------------------

pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub aspect_ratio: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    /// Computes view-projection matrix compatible with Wgpu / Vulkan depth clip [0, 1].
    pub fn build_view_projection_matrix(&self) -> Mat4 {
        let view = look_at_mat4(self.eye, self.target, self.up);
        let proj = perspective(self.fovy, self.aspect_ratio, self.znear, self.zfar);
        proj * view
    }
}

// ---------------------------------------------------------------------------
// Camera Uniform GPU Buffer
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_position: [f32; 4],
    proj_view: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        Self {
            view_position: [0.0; 4],
            proj_view: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    fn update_view_proj(&mut self, camera: &Camera) {
        self.view_position = camera.eye.to_homogeneous().to_array();
        self.proj_view = camera.build_view_projection_matrix().to_cols_array_2d();
    }
}

// ---------------------------------------------------------------------------
// Camera Controller (WASD + Space/Shift + Arrows)
// ---------------------------------------------------------------------------

struct CameraController {
    speed: f32,
    is_up_pressed: bool,
    is_down_pressed: bool,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
}

impl CameraController {
    fn new(speed: f32) -> Self {
        Self {
            speed,
            is_up_pressed: false,
            is_down_pressed: false,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
        }
    }

    fn handle_key(&mut self, key: KeyCode, is_pressed: bool) -> bool {
        match key {
            KeyCode::Space => {
                self.is_up_pressed = is_pressed;
                true
            }
            KeyCode::ShiftLeft => {
                self.is_down_pressed = is_pressed;
                true
            }
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.is_forward_pressed = is_pressed;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.is_left_pressed = is_pressed;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.is_backward_pressed = is_pressed;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.is_right_pressed = is_pressed;
                true
            }
            _ => false,
        }
    }

    fn update_camera(&self, camera: &mut Camera) {
        let forward = camera.target - camera.eye;
        let forward_norm = forward.normalize();
        let forward_mag = forward.length();

        // Prevents camera glitching when moving too close to the target
        if self.is_forward_pressed && forward_mag > self.speed {
            camera.eye += forward_norm * self.speed;
        }
        if self.is_backward_pressed {
            camera.eye -= forward_norm * self.speed;
        }

        let right = forward_norm.cross(camera.up).normalize();

        // Recalculate forward vector after linear motion
        let forward = camera.target - camera.eye;
        let forward_mag = forward.length();

        if self.is_right_pressed {
            // Orbit camera around target maintaining radius
            camera.eye = camera.target - (forward + right * self.speed).normalize() * forward_mag;
        }
        if self.is_left_pressed {
            camera.eye = camera.target - (forward - right * self.speed).normalize() * forward_mag;
        }

        if self.is_up_pressed {
            camera.eye += camera.up * self.speed;
        }
        if self.is_down_pressed {
            camera.eye -= camera.up * self.speed;
        }
    }
}

// ---------------------------------------------------------------------------
// Instance representation and GPU layout
// ---------------------------------------------------------------------------

struct Instance {
    position: Vec3,
    rotation: Quat,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
}

impl Instance {
    fn to_raw(&self) -> InstanceRaw {
        let model = Mat4::from_translation(self.position) * Mat4::from_quat(self.rotation);
        // Extract the 3x3 normal matrix (if scale is uniform, rotation-only)
        let normal = Mat3::from_quat(self.rotation);
        InstanceRaw {
            model: model.to_cols_array_2d(),
            normal: normal.to_cols_array_2d(),
        }
    }
}

impl model::Vertex for InstanceRaw {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // Model matrix column 0 (slot 5)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Model matrix column 1 (slot 6)
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Model matrix column 2 (slot 7)
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Model matrix column 3 (slot 8)
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Normal matrix column 4 (slot 9)
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal matrix column 5 (slot 10)
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal matrix column 6 (slot 11)
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

// ------------------------------------------------------------------------------
// Light implementation using Blinn-Phong model represent colored point in space
// ------------------------------------------------------------------------------
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniform {
    position: [f32; 3],
    // Due to uniforms requiring 16 bytes (4 float) spacing, we need to use padding field here
    _padding: u32,
    color: [f32; 3],
    // Due to uniforms requiring 16 bytes (4 float) spacing, we need to use padding field here
    _padding2: u32,
}

// ---------------------------------------------------------------------------
// Application State
// ---------------------------------------------------------------------------

struct State {
    render_pipeline: wgpu::RenderPipeline,
    obj_model: model::Model,
    camera: Camera,
    camera_controller: CameraController,
    camera_buffer: wgpu::Buffer,
    camera_uniform: CameraUniform,
    camera_bind_group: wgpu::BindGroup,
    instances: Vec<Instance>,
    instance_buffer: wgpu::Buffer,
    depth_texture: texture::Texture,
    light_uniform: LightUniform,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    light_render_pipeline: wgpu::RenderPipeline,
}
fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[Option<wgpu::VertexBufferLayout>],
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: vertex_layouts,
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            // Required Features::DEPTH_CLIP_CONTROL
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            // Requires Features::CONSERVATIVE_RASTERIZATION
            conservative: false,
        },
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
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
                format: color_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
impl App for State {
    fn title() -> &'static str {
        "Tutorial 8 - Model Loading"
    }

    fn init(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
    ) -> anyhow::Result<Self> {
        // 1. Texture bind group layout for materials (group 0)
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                entries: &[
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // 2. Camera setup
        let camera = Camera {
            eye: Vec3::new(0.0, 5.0, -10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            aspect_ratio: config.width as f32 / config.height as f32,
            fovy: 45_f32.to_radians(),
            znear: 0.1,
            zfar: 100.0,
        };

        let camera_controller = CameraController::new(0.2);
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Camera bind group layout (group 1)
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // 3. Light setup (group 2)
        let light_uniform = LightUniform {
            position: [2.0, 2.0, 2.0],
            _padding: 0,
            color: [1.0, 1.0, 1.0], // White light
            _padding2: 0,
        };

        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Buffer"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("light_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light_bind_group"),
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
        });

        // 4. Load 3D model and its materials asynchronously using pollster
        let obj_model = pollster::block_on(resources::load_gltf_model(
            "cube.gltf",
            device,
            queue,
            &texture_bind_group_layout,
        ))?;

        // 5. Create instances on a grid
        const SPACE_BETWEEN: f32 = 3.0;
        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                    let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                    let position = Vec3::new(x, 0.0, z);

                    let rotation = if position == Vec3::ZERO {
                        Quat::from_axis_angle(Vec3::Z, 0.0)
                    } else {
                        Quat::from_axis_angle(position.normalize(), 45_f32.to_radians())
                    };

                    Instance { position, rotation }
                })
            })
            .collect::<Vec<_>>();

        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        // 6. Depth texture
        let depth_texture = texture::Texture::create_depth(device, config, "depth_texture");

        // 7. Normal Shader Pipeline
        let render_pipeline = {
            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[
                        Some(&texture_bind_group_layout),
                        Some(&camera_bind_group_layout),
                        Some(&light_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Normal Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("./shader.wgsl").into()),
            };

            create_render_pipeline(
                device,
                &render_pipeline_layout,
                config.format,
                Some(texture::Texture::DEPTH_FORMAT),
                &[Some(model::ModelVertex::desc()), Some(InstanceRaw::desc())],
                shader,
            )
        };

        // 8. Light render pipeline
        let light_render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&camera_bind_group_layout),
                    Some(&light_bind_group_layout),
                ],
                immediate_size: 0,
            });

            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Light Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("./light.wgsl").into()),
            };

            create_render_pipeline(
                device,
                &layout,
                config.format,
                Some(texture::Texture::DEPTH_FORMAT),
                &[Some(model::ModelVertex::desc())],
                shader,
            )
        };

        Ok(Self {
            render_pipeline,
            obj_model,
            camera,
            camera_controller,
            camera_buffer,
            camera_uniform,
            camera_bind_group,
            instances,
            instance_buffer,
            depth_texture,
            light_uniform,
            light_buffer,
            light_bind_group,
            light_render_pipeline,
        })
    }

    fn render(&mut self, ctx: &framework::Context, view: &wgpu::TextureView) -> anyhow::Result<()> {
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.008,
                            g: 0.007,
                            b: 0.050,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Bind instance buffer to slot 1
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            use crate::model::DrawLight;
            render_pass.set_pipeline(&self.light_render_pipeline);
            render_pass.draw_light_model(
                &self.obj_model,
                &self.camera_bind_group,
                &self.light_bind_group,
            );

            render_pass.set_pipeline(&self.render_pipeline);
            // Draw all instances using the DrawModel trait extension
            render_pass.draw_model_instanced(
                &self.obj_model,
                0..self.instances.len() as u32,
                &self.camera_bind_group,
                &self.light_bind_group,
            );
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    fn resize(&mut self, ctx: &mut framework::Context, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.camera.aspect_ratio = width as f32 / height as f32;
            self.depth_texture =
                texture::Texture::create_depth(&ctx.device, &ctx.config, "depth_texture");
            ctx.resize(width, height);
        }
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        if code == KeyCode::Escape && is_pressed {
            event_loop.exit();
        } else {
            self.camera_controller.handle_key(code, is_pressed);
        }
    }

    fn update(&mut self, ctx: &framework::Context) {
        // Update camera position from input
        self.camera_controller.update_camera(&mut self.camera);
        self.camera_uniform.update_view_proj(&self.camera);

        ctx.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        // Update the light - rotates around the Y-axis smoothly
        let old_pos = Vec3::from_slice(&self.light_uniform.position);
        let rot = Quat::from_axis_angle(Vec3::Y, 1_f32.to_radians());
        let new_pos = rot * old_pos;
        self.light_uniform.position = new_pos.to_array();

        ctx.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );
    }
}

fn main() -> anyhow::Result<()> {
    framework::run::<State>()
}
