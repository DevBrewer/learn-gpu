// Demo 3D Blinn-Phong + Normal Mapping Shader

struct Camera {
    view_pos: vec4<f32>,
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
}

struct Light {
    position: vec3<f32>,
    _pad0: u32,
    color: vec3<f32>,
    _pad1: u32,
}

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(0) @binding(2) var t_normal: texture_2d<f32>;
@group(0) @binding(3) var s_normal: sampler;

@group(1) @binding(0) var<uniform> camera: Camera;
@group(2) @binding(0) var<uniform> light: Light;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) world_bitangent: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.tex_coords = in.tex_coords;
    out.world_position = in.position;
    out.world_normal = in.normal;
    out.world_tangent = in.tangent;
    out.world_bitangent = in.bitangent;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let object_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    let normal_map_sample = textureSample(t_normal, s_normal, in.tex_coords).xyz * 2.0 - 1.0;

    let TBN = mat3x3<f32>(
        normalize(in.world_tangent),
        normalize(in.world_bitangent),
        normalize(in.world_normal),
    );
    let N = normalize(TBN * normal_map_sample);

    let light_dir = normalize(light.position - in.world_position);
    let view_dir = normalize(camera.view_pos.xyz - in.world_position);
    let half_dir = normalize(view_dir + light_dir);

    let ambient = vec3<f32>(0.1, 0.1, 0.15);
    let diffuse_strength = max(dot(N, light_dir), 0.0);
    let diffuse = light.color * diffuse_strength;

    let specular_strength = pow(max(dot(N, half_dir), 0.0), 32.0);
    let specular = light.color * specular_strength * 0.5;

    let final_color = (ambient + diffuse + specular) * object_color.rgb;
    return vec4<f32>(final_color, object_color.a);
}
