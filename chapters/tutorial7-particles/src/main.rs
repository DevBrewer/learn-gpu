#![allow(unused)]
use framework::App;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;
use winit::{event::MouseButton, event_loop::ActiveEventLoop, keyboard::KeyCode};

// ============================================================================
// 1. VERTEX DEFINITIONS & GEOMETRY
// ============================================================================

/// Local 2D vertex for camera-facing billboard particle quads.
/// In view-space billboarding, the quad is extruded in the camera's local X/Y plane.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ParticleVertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

impl ParticleVertex {
    /// Describes the GPU vertex layout for billboard quads (2D position + UV coords).
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Location 0: Local 2D quad position [-1.0, 1.0]
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                // Location 1: UV coordinates [0.0, 1.0] for circular glow
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                },
            ],
        }
    }
}

/// Normalized unit quad vertices [-1.0, 1.0]. The instance `scale` field directly
/// sets the actual world-space radius of each particle spark.
const PARTICLE_VERTICES: &[ParticleVertex] = &[
    ParticleVertex {
        position: [-1.0, 1.0],
        tex_coords: [0.0, 0.0],
    }, // Top-left
    ParticleVertex {
        position: [-1.0, -1.0],
        tex_coords: [0.0, 1.0],
    }, // Bottom-left
    ParticleVertex {
        position: [1.0, -1.0],
        tex_coords: [1.0, 1.0],
    }, // Bottom-right
    ParticleVertex {
        position: [1.0, 1.0],
        tex_coords: [1.0, 0.0],
    }, // Top-right
];

const PARTICLE_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

/// 3D vertex with RGBA color used for rendering the ground perspective reference grid.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GridVertex {
    position: [f32; 3],
    color: [f32; 4],
}

impl GridVertex {
    /// Describes the GPU vertex layout for 3D grid line segments.
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GridVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Location 0: 3D world position
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // Location 1: Line RGBA color
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                },
            ],
        }
    }
}

/// Generates a 3D ground reference grid at Y = -2.5 with a launch pad circle
/// to give clear spatial depth and perspective reference.
fn build_ground_grid() -> Vec<GridVertex> {
    let mut vertices = Vec::new();
    let y = -2.5;
    let extent = 5.0;
    let step = 0.5;
    let grid_color = [0.12, 0.20, 0.32, 0.45];
    let axis_color = [0.22, 0.38, 0.55, 0.70];

    // Grid lines along X and Z axes
    let mut coord: f32 = -extent;
    while coord <= extent + 0.001 {
        let is_axis = coord.abs() < 0.01;
        let col = if is_axis { axis_color } else { grid_color };

        // Line parallel to Z
        vertices.push(GridVertex {
            position: [coord, y, -extent],
            color: col,
        });
        vertices.push(GridVertex {
            position: [coord, y, extent],
            color: col,
        });

        // Line parallel to X
        vertices.push(GridVertex {
            position: [-extent, y, coord],
            color: col,
        });
        vertices.push(GridVertex {
            position: [extent, y, coord],
            color: col,
        });

        coord += step;
    }

    // Concentric launch pad rings on the ground plane
    for &radius in &[1.0, 2.5, 4.0] {
        let segments = 64;
        for i in 0..segments {
            let theta1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let theta2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
            let col = [0.15, 0.28, 0.45, 0.55];

            vertices.push(GridVertex {
                position: [radius * theta1.cos(), y, radius * theta1.sin()],
                color: col,
            });
            vertices.push(GridVertex {
                position: [radius * theta2.cos(), y, radius * theta2.sin()],
                color: col,
            });
        }
    }

    vertices
}

// ============================================================================
// 2. DATA-ORIENTED DESIGN (SoA) PARTICLE SYSTEM
// ============================================================================

/// Structure of Arrays (SoA) Particle System.
/// Stores particle attributes in parallel contiguous vectors for cache locality and SIMD execution.
#[derive(Default)]
pub struct ParticleSystemSoA {
    pub positions: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    pub lifespans: Vec<f32>,
    pub colors: Vec<[f32; 4]>,
    pub is_rocket: Vec<bool>,
    pub trail_history: Vec<[Vec3; 6]>,
    pub trail_counts: Vec<usize>,
}

impl ParticleSystemSoA {
    /// Creates an empty SoA particle system with preallocated capacity to avoid reallocations.
    pub fn new() -> Self {
        Self {
            positions: Vec::with_capacity(4096),
            velocities: Vec::with_capacity(4096),
            lifespans: Vec::with_capacity(4096),
            colors: Vec::with_capacity(4096),
            is_rocket: Vec::with_capacity(4096),
            trail_history: Vec::with_capacity(4096),
            trail_counts: Vec::with_capacity(4096),
        }
    }

    /// Returns the current total count of active particles (rockets + embers).
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Spawns an ascending firework rocket at ground level with random upward velocity and HSV color.
    pub fn spawn_rocket(&mut self) {
        let start_x = (rand::random::<f32>() - 0.5) * 4.0;
        let start_z = (rand::random::<f32>() - 0.5) * 4.0;
        let hue = rand::random::<f32>() * 360.0;
        let rgb = hsv_to_rgb(hue, 1.0, 1.0);

        self.positions.push(Vec3::new(start_x, -2.5, start_z));
        self.velocities.push(Vec3::new(
            (rand::random::<f32>() - 0.5) * 0.006,
            rand::random::<f32>() * 0.025 + 0.075,
            (rand::random::<f32>() - 0.5) * 0.006,
        ));
        self.lifespans.push(255.0);
        self.colors.push([rgb[0], rgb[1], rgb[2], 1.0]);
        self.is_rocket.push(true);
        self.trail_history.push([Vec3::ZERO; 6]);
        self.trail_counts.push(0);
    }

    /// Explodes an apex rocket into a spherical burst of embers using uniform 3D sphere sampling.
    fn explode(&mut self, index: usize) {
        let center = self.positions[index];
        let color = self.colors[index];
        let spark_count = 85;

        for _ in 0..spark_count {
            // Archimedes' uniform spherical surface sampling
            let u: f32 = rand::random::<f32>() * 2.0 - 1.0;
            let phi_sin = (1.0 - u * u).max(0.0).sqrt();
            let theta = rand::random::<f32>() * std::f32::consts::TAU;

            // Tight speed range for small, crisp explosion radius
            let speed = rand::random::<f32>() * 0.028 + 0.012;

            let vel = Vec3::new(
                phi_sin * theta.cos() * speed,
                u * speed,
                phi_sin * theta.sin() * speed,
            );

            self.positions.push(center);
            self.velocities.push(vel);
            self.lifespans.push(255.0);
            self.colors.push(color);
            self.is_rocket.push(false);
            self.trail_history.push([center; 6]);
            self.trail_counts.push(0);
        }
    }

    /// Removes a particle at `index` in O(1) time by swapping with the last element across all SoA arrays.
    fn swap_remove(&mut self, index: usize) {
        self.positions.swap_remove(index);
        self.velocities.swap_remove(index);
        self.lifespans.swap_remove(index);
        self.colors.swap_remove(index);
        self.is_rocket.swap_remove(index);
        self.trail_history.swap_remove(index);
        self.trail_counts.swap_remove(index);
    }

    /// Advances particle physics (gravity, drag, aging, trail history) and triggers explosions at apex.
    pub fn update(&mut self, gravity: Vec3) {
        let mut to_explode = Vec::new();

        // Process backwards so swap_remove does not invalidate upcoming indices
        for i in (0..self.positions.len()).rev() {
            // Update trail position history ring buffer
            let max_trail = if self.is_rocket[i] { 6 } else { 4 };
            let count = (self.trail_counts[i] + 1).min(max_trail);
            self.trail_counts[i] = count;

            for t in (1..count).rev() {
                self.trail_history[i][t] = self.trail_history[i][t - 1];
            }
            self.trail_history[i][0] = self.positions[i];

            if self.is_rocket[i] {
                // Rocket physics: ascend under negative gravity
                self.velocities[i] += gravity;
                self.positions[i] += self.velocities[i];

                // Explode at peak (vertical velocity drops to <= 0.0)
                if self.velocities[i].y <= 0.0 {
                    to_explode.push(i);
                }
            } else {
                // Explosion embers: air resistance + gravity + lifespan decay
                self.velocities[i] *= 0.96;
                self.velocities[i] += gravity;
                self.positions[i] += self.velocities[i];
                self.lifespans[i] -= 3.8;

                if self.lifespans[i] <= 0.0 {
                    self.swap_remove(i);
                }
            }
        }

        // Explode triggered rockets
        for idx in to_explode {
            if idx < self.positions.len() && self.is_rocket[idx] {
                self.explode(idx);
                self.swap_remove(idx);
            }
        }
    }

    /// Generates compact per-instance data (position, scale, color) for particles and their motion trails.
    pub fn write_instances(&self, instances: &mut Vec<InstanceRaw>) {
        instances.clear();

        for i in 0..self.positions.len() {
            let is_rocket = self.is_rocket[i];
            let alpha = if is_rocket {
                1.0
            } else {
                (self.lifespans[i] / 255.0).clamp(0.0, 1.0)
            };

            // Small, crisp particle radius (world units)
            let base_scale = if is_rocket { 0.022 } else { 0.015 * alpha };

            // 1. Primary particle instance
            let mut col = self.colors[i];
            col[3] = alpha;
            instances.push(InstanceRaw {
                position: self.positions[i].to_array(),
                scale: base_scale,
                color: col,
            });

            // 2. Motion trail streamer instances
            let trail_len = self.trail_counts[i];
            for t in 0..trail_len {
                let progress = 1.0 - (t as f32 / trail_len as f32);
                let trail_alpha = alpha * progress * 0.6;
                let trail_scale = base_scale * (progress * 0.7 + 0.1);
                let mut trail_col = self.colors[i];
                trail_col[3] = trail_alpha;

                instances.push(InstanceRaw {
                    position: self.trail_history[i][t].to_array(),
                    scale: trail_scale,
                    color: trail_col,
                });
            }
        }
    }
}

/// Converts HSV color (hue 0..360, saturation 0..1, value 0..1) into normalized linear RGB.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

// ============================================================================
// 3. INSTANCE BUFFER DATA LAYOUT
// ============================================================================

/// Compact per-instance GPU data (32 bytes total).
/// Replaces 64-byte 4x4 matrix with world position + scalar radius for view-space billboarding.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    pub position: [f32; 3], // 12 bytes
    pub scale: f32,         // 4 bytes
    pub color: [f32; 4],    // 16 bytes
}

impl InstanceRaw {
    /// Describes the GPU per-instance step layout (position at loc 5, scale at loc 6, color at loc 7).
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // Location 5: 3D world position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Location 6: Particle scale (radius)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32,
                },
                // Location 7: Particle RGBA color
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() + std::mem::size_of::<f32>())
                        as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

// ============================================================================
// 4. 3D ORBIT CAMERA & UNIFORMS
// ============================================================================

/// 3D perspective camera holding position, target, FOV, and clip planes.
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
    /// Computes the right-handed view matrix (camera world transform to eye space).
    fn build_view_matrix(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.eye, self.target, self.up)
    }

    /// Computes the DirectX-style perspective projection matrix matching WGPU's NDC (+Y up, Z in [0, 1]).
    fn build_proj_matrix(&self) -> Mat4 {
        glam::camera::rh::proj::directx::perspective(
            self.fovy,
            self.aspect_ratio,
            self.znear,
            self.zfar,
        )
    }
}

/// Uniform buffer struct uploaded to group 0, binding 0 containing separate view and projection matrices.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
}

impl CameraUniform {
    /// Initializes uniform matrices with identity values.
    fn new() -> Self {
        Self {
            view: Mat4::IDENTITY.to_cols_array_2d(),
            proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    /// Recomputes view and projection matrices from current camera state.
    fn update(&mut self, camera: &Camera) {
        self.view = camera.build_view_matrix().to_cols_array_2d();
        self.proj = camera.build_proj_matrix().to_cols_array_2d();
    }
}

/// Spherical orbit camera controller supporting mouse dragging, keyboard tilt, and auto-rotation.
struct CameraController {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
    auto_rotate: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    is_up_pressed: bool,
    is_down_pressed: bool,
    last_mouse_pos: Option<(f32, f32)>,
    mouse_dragging: bool,
}

impl CameraController {
    /// Initializes the orbit controller with elevated pitch angle and auto-orbit enabled.
    fn new() -> Self {
        Self {
            yaw: 0.2,
            pitch: 0.25, // Elevated view angle to showcase 3D ground plane and particle altitudes
            distance: 8.0,
            target: Vec3::new(0.0, 0.2, 0.0),
            auto_rotate: true,
            is_left_pressed: false,
            is_right_pressed: false,
            is_up_pressed: false,
            is_down_pressed: false,
            last_mouse_pos: None,
            mouse_dragging: false,
        }
    }

    /// Handles keyboard events for manual orbit control and toggling auto-rotation (Space).
    fn handle_key(&mut self, key: KeyCode, is_pressed: bool) -> bool {
        match key {
            KeyCode::Space if is_pressed => {
                self.auto_rotate = !self.auto_rotate;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.is_left_pressed = is_pressed;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.is_right_pressed = is_pressed;
                true
            }
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.is_up_pressed = is_pressed;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.is_down_pressed = is_pressed;
                true
            }
            _ => false,
        }
    }

    /// Updates yaw and pitch angles when the user drags with the mouse.
    fn handle_mouse_move(&mut self, x: f32, y: f32) {
        if self.mouse_dragging
            && let Some((last_x, last_y)) = self.last_mouse_pos
        {
            let dx = x - last_x;
            let dy = y - last_y;
            self.yaw += dx * 0.005;
            self.pitch = (self.pitch - dy * 0.005).clamp(-1.3, 1.3);
        }
        self.last_mouse_pos = Some((x, y));
    }

    /// Begins or ends mouse drag orbiting on left mouse click/release.
    fn handle_mouse_button(&mut self, button: MouseButton, is_pressed: bool) {
        if button == MouseButton::Left {
            self.mouse_dragging = is_pressed;
            if is_pressed {
                self.auto_rotate = false; // Pause auto-rotation during user interaction
            }
        }
    }

    /// Computes eye position in 3D Cartesian coordinates from spherical yaw, pitch, and distance.
    fn update_camera(&mut self, camera: &mut Camera) {
        if self.auto_rotate && !self.mouse_dragging {
            self.yaw += 0.004;
        }

        if self.is_left_pressed {
            self.yaw -= 0.025;
            self.auto_rotate = false;
        }
        if self.is_right_pressed {
            self.yaw += 0.025;
            self.auto_rotate = false;
        }
        if self.is_up_pressed {
            self.pitch = (self.pitch + 0.02).min(1.3);
            self.auto_rotate = false;
        }
        if self.is_down_pressed {
            self.pitch = (self.pitch - 0.02).max(-1.3);
            self.auto_rotate = false;
        }

        // Convert spherical angles to 3D position
        let x = self.distance * self.pitch.cos() * self.yaw.sin();
        let y = self.distance * self.pitch.sin();
        let z = self.distance * self.pitch.cos() * self.yaw.cos();

        camera.eye = self.target + Vec3::new(x, y, z);
        camera.target = self.target;
    }
}

// ============================================================================
// 5. APPLICATION STATE AND PIPELINES
// ============================================================================

struct State {
    particle_pipeline: wgpu::RenderPipeline,
    particle_vertex_buffer: wgpu::Buffer,
    particle_index_buffer: wgpu::Buffer,
    grid_pipeline: wgpu::RenderPipeline,
    grid_vertex_buffer: wgpu::Buffer,
    grid_vertex_count: u32,
    depth_texture_view: wgpu::TextureView,
    num_particle_indices: u32,
    num_instances: u32,
    camera: Camera,
    camera_controller: CameraController,
    camera_buffer: wgpu::Buffer,
    camera_uniform: CameraUniform,
    camera_bind_group: wgpu::BindGroup,
    particle_system: ParticleSystemSoA,
    instance_buffer: wgpu::Buffer,
    instance_scratch: Vec<InstanceRaw>,
}

impl State {
    /// Allocates a Depth32Float texture matching the current window dimensions for 3D depth testing.
    fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::TextureView {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        depth_texture.create_view(&wgpu::TextureViewDescriptor::default())
    }
}

impl App for State {
    /// Window title displayed in the OS window header.
    fn title() -> &'static str {
        "WGPU 3D Fireworks Simulation (Data-Oriented SoA)"
    }

    /// Initializes GPU resources: buffers, shaders, depth textures, and pipelines.
    fn init(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
    ) -> anyhow::Result<Self> {
        // --- 1. GEOMETRY BUFFERS ---
        let particle_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Vertex Buffer"),
            contents: bytemuck::cast_slice(PARTICLE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let particle_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Index Buffer"),
            contents: bytemuck::cast_slice(PARTICLE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let num_particle_indices = PARTICLE_INDICES.len() as u32;

        let grid_vertices = build_ground_grid();
        let grid_vertex_count = grid_vertices.len() as u32;
        let grid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Grid Vertex Buffer"),
            contents: bytemuck::cast_slice(&grid_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // --- 2. CAMERA SETUP ---
        let mut camera = Camera {
            eye: Vec3::new(0.0, 2.0, 8.0),
            target: Vec3::new(0.0, 0.2, 0.0),
            up: Vec3::Y,
            aspect_ratio: config.width as f32 / config.height as f32,
            fovy: 45_f32.to_radians(),
            znear: 0.1,
            zfar: 100.0,
        };

        let mut camera_controller = CameraController::new();
        let mut camera_uniform = CameraUniform::new();
        camera_controller.update_camera(&mut camera);
        camera_uniform.update(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // --- 3. DYNAMIC INSTANCE BUFFER (UP TO 20,000 INSTANCES) ---
        let max_instances = 20000;
        let dummy_instances = vec![
            InstanceRaw {
                position: [0.0; 3],
                scale: 0.0,
                color: [0.0; 4],
            };
            max_instances
        ];

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&dummy_instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        // --- 4. DEPTH TEXTURE ---
        let depth_texture_view = Self::create_depth_texture(device, config);

        // --- 5. SHADERS AND PIPELINES ---
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle and Grid Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[Some(&camera_bind_group_layout)],
            immediate_size: 0,
        });

        // 3D Ground Grid Pipeline (Depth write enabled)
        let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Grid Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_grid"),
                buffers: &[Some(GridVertex::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_grid"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Particle Billboard Pipeline (Depth test enabled, depth write disabled for alpha blending)
        let particle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(ParticleVertex::desc()), Some(InstanceRaw::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // --- 6. INITIALIZE SoA PARTICLE SYSTEM ---
        let mut particle_system = ParticleSystemSoA::new();
        particle_system.spawn_rocket();

        Ok(Self {
            particle_pipeline,
            particle_vertex_buffer,
            particle_index_buffer,
            grid_pipeline,
            grid_vertex_buffer,
            grid_vertex_count,
            depth_texture_view,
            num_particle_indices,
            num_instances: 0,
            camera,
            camera_controller,
            camera_buffer,
            camera_uniform,
            camera_bind_group,
            particle_system,
            instance_buffer,
            instance_scratch: Vec::with_capacity(16384),
        })
    }

    /// Per-frame simulation loop: advances camera, spawns rockets, updates physics, and streams instance data.
    fn update(&mut self, _ctx: &framework::Context) {
        // Update 3D Orbit Camera
        self.camera_controller.update_camera(&mut self.camera);
        self.camera_uniform.update(&self.camera);

        // Upload updated camera matrices to uniform buffer
        _ctx.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        // Physics downward gravity vector
        let gravity = Vec3::new(0.0, -0.0022, 0.0);

        // Occasionally spawn new rockets across the 3D ground plane
        let active_rockets = self
            .particle_system
            .is_rocket
            .iter()
            .filter(|&&r| r)
            .count();
        if rand::random::<f32>() < 0.05 && active_rockets < 6 {
            self.particle_system.spawn_rocket();
        }

        // Update SoA particle physics in parallel
        self.particle_system.update(gravity);

        // Extract instance data
        self.particle_system
            .write_instances(&mut self.instance_scratch);
        self.num_instances = self.instance_scratch.len() as u32;

        // Upload dynamic instances to GPU
        if !self.instance_scratch.is_empty() {
            _ctx.queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&self.instance_scratch),
            );
        }
    }

    /// Handles window resizing by updating camera aspect ratio and recreating the depth texture.
    fn resize(&mut self, ctx: &mut framework::Context, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.camera.aspect_ratio = width as f32 / height as f32;
            self.depth_texture_view = Self::create_depth_texture(&ctx.device, &ctx.config);
        }
    }

    /// Records and submits render commands: draws the 3D ground grid and view-facing particle billboards.
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
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Night sky deep black/slate background
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.04,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            // 1. Draw 3D Ground Perspective Grid
            render_pass.set_pipeline(&self.grid_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
            render_pass.draw(0..self.grid_vertex_count, 0..1);

            // 2. Draw 3D Particles with View-Space Billboarding
            if self.num_instances > 0 {
                render_pass.set_pipeline(&self.particle_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.particle_vertex_buffer.slice(..));
                render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
                render_pass.set_index_buffer(
                    self.particle_index_buffer.slice(..),
                    wgpu::IndexFormat::Uint16,
                );
                render_pass.draw_indexed(0..self.num_particle_indices, 0, 0..self.num_instances);
            }
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Handles keyboard input: Escape to quit, WASD/Arrows to orbit/tilt, and Space to toggle auto-rotation.
    fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        if code == KeyCode::Escape && is_pressed {
            event_loop.exit();
        } else {
            self.camera_controller.handle_key(code, is_pressed);
        }
    }

    /// Forwards cursor movement to the orbit camera controller.
    fn handle_mouse_move(&mut self, x: f32, y: f32) {
        self.camera_controller.handle_mouse_move(x, y);
    }

    /// Forwards mouse click events to initiate drag orbiting.
    fn mouse_pressed(&mut self, _ctx: &framework::Context, button: MouseButton) {
        self.camera_controller.handle_mouse_button(button, true);
    }

    /// Forwards mouse release events to stop drag orbiting.
    fn mouse_release(&mut self, _ctx: &framework::Context, button: MouseButton) {
        self.camera_controller.handle_mouse_button(button, false);
    }
}

/// Application entry point: initializes environment logger and starts the Winit event loop.
fn main() -> anyhow::Result<()> {
    framework::run::<State>()
}

// ============================================================================
// 6. UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Validates that both particle and grid WGSL shaders parse and validate cleanly with Naga.
    #[test]
    fn test_shader_compiles() {
        let shader = include_str!("./shader.wgsl");
        let module = wgpu::naga::front::wgsl::parse_str(shader);
        if let Err(e) = module {
            panic!("Shader failed to compile: {:?}", e);
        }
    }

    /// Verifies that perspective matrix Y-axis points upwards (+Y) matching WGPU NDC.
    #[test]
    fn test_projection_y_points_up() {
        let proj = glam::camera::rh::proj::directx::perspective(1.0, 1.0, 0.1, 100.0);
        assert!(
            proj.y_axis.y > 0.0,
            "Projection Y axis should point UP in WGPU!"
        );
    }

    /// Tests SoA particle system lifecycle: spawning, simulation stepping, and apex explosion.
    #[test]
    fn test_soa_particle_system() {
        let mut ps = ParticleSystemSoA::new();
        assert_eq!(ps.len(), 0);

        ps.spawn_rocket();
        assert_eq!(ps.len(), 1);
        assert!(ps.is_rocket[0]);

        // Advance physics until apex explosion occurs
        for _ in 0..100 {
            ps.update(Vec3::new(0.0, -0.01, 0.0));
            if !ps.is_rocket.is_empty() && !ps.is_rocket[0] {
                break;
            }
        }

        // Rocket should have exploded into multiple child sparks
        assert!(ps.len() > 1);

        let mut instances = Vec::new();
        ps.write_instances(&mut instances);
        assert!(!instances.is_empty());
    }
}
