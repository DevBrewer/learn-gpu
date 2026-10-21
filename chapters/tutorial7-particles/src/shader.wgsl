struct CameraUniform {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
}
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// ============================================================================
// PARTICLE BILLBOARD SHADER (Camera-facing view-space billboarding)
// ============================================================================

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct InstanceInput {
    @location(5) world_pos: vec3<f32>,
    @location(6) scale: f32,
    @location(7) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
    // 1. Transform particle center to camera view space
    let view_center = camera.view * vec4<f32>(instance.world_pos, 1.0);
    // 2. Extrude the quad in view space so billboard always faces the camera in 3D
    let view_pos = view_center.xyz + vec3<f32>(model.position * instance.scale, 0.0);

    var out: VertexOutput;
    out.clip_position = camera.proj * vec4<f32>(view_pos, 1.0);
    out.tex_coords = model.tex_coords;
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Distance from the center of the billboard quad (0.5, 0.5)
    let dist = distance(in.tex_coords, vec2<f32>(0.5, 0.5));
    if (dist > 0.5) {
        discard;
    }
    // Smooth radial glow falloff from center (1.0) to edge (0.0)
    let factor = smoothstep(0.5, 0.0, dist);
    return vec4<f32>(in.color.rgb, in.color.a * factor);
}

// ============================================================================
// 3D GROUND REFERENCE GRID SHADER
// ============================================================================

struct GridVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct GridVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_grid(in: GridVertexInput) -> GridVertexOutput {
    var out: GridVertexOutput;
    out.clip_position = camera.proj * camera.view * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_grid(in: GridVertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
