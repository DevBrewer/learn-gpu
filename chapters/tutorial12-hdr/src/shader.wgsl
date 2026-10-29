// ---------------------------------------------------------------------------
// Uniforms
// ---------------------------------------------------------------------------
// Point light parameters in world space (Group 2, Binding 0)
struct Light {
    position: vec3f,
    color: vec3f,
}
@group(2) @binding(0)
var<uniform> light: Light;

// Camera view-projection and world position (Group 1, Binding 0)
struct Camera {
    view_pos: vec3<f32>,
    view:mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj:mat4x4<f32>,
    inv_view:mat4x4<f32>,
}
@group(1) @binding(0)
var<uniform> camera: Camera;

// ---------------------------------------------------------------------------
// Vertex & Instance Inputs
// ---------------------------------------------------------------------------
// Per-vertex attributes uploaded from CPU vertex buffers:
// Locations 3 & 4 provide the Tangent and Bitangent calculated per triangle face.
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
}

// Per-instance attributes (columns of the 4x4 model matrix and 3x3 normal matrix)
struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
    @location(9) normal_matrix_0: vec3<f32>,
    @location(10) normal_matrix_1: vec3<f32>,
    @location(11) normal_matrix_2: vec3<f32>,
}

// Interpolated outputs passed from vertex shader to fragment shader:
// World-space coordinates are pre-transformed into Tangent Space in the vertex
// shader so that lighting calculations in the fragment shader stay cheap.
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) world_view_position: vec3<f32>,
    @location(3) world_light_position: vec3<f32>,
    @location(4) world_normal: vec3<f32>,
    @location(5) world_tangent: vec3<f32>,
    @location(6) world_bitangent: vec3<f32>,
}

// ---------------------------------------------------------------------------
// Vertex Shader
// ---------------------------------------------------------------------------
@vertex
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Reconstruct 4x4 model matrix from instance columns
    let model_matrix = mat4x4<f32>(
            instance.model_matrix_0,
            instance.model_matrix_1,
            instance.model_matrix_2,
            instance.model_matrix_3,
        );

    // Reconstruct 3x3 normal matrix (inverse-transpose of model matrix)
    let normal_matrix = mat3x3<f32>(
            instance.normal_matrix_0,
            instance.normal_matrix_1,
            instance.normal_matrix_2,
        );

    // Calculate vertex position in world space
    let world_position = model_matrix * vec4<f32>(model.position, 1.0);

    var out: VertexOutput;
    // Transform normal, tangent, and bitangent to world space and normalize
    out.tex_coords = model.tex_coords;
    // Project position to clip space for rasterization
    out.clip_position = camera.view_proj * world_position;
    out.tex_coords = model.tex_coords;
    out.world_normal = normalize(normal_matrix * model.normal);
    out.world_tangent = normalize(normal_matrix * model.tangent);
    out.world_bitangent = normalize(normal_matrix * model.bitangent);
    out.world_position = world_position.xyz;
    out.world_view_position=camera.view_pos.xyz;


    return out;
}

// ---------------------------------------------------------------------------
// Fragment Shader
// ---------------------------------------------------------------------------

// Material textures and samplers (Group 0, Bindings 0..=3)
@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var t_normal: texture_2d<f32>;
@group(0) @binding(3)
var s_normal: sampler;

@group(3) @binding(0)
var env_map: texture_cube<f32>;
@group(3) @binding(1)
var env_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample base diffuse texture (albedo)
    let object_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);

    // Sample normal map: colors [0.0, 1.0] are mapped to tangent-space vector [-1.0, 1.0]
    let object_normal = textureSample(t_normal, s_normal, in.tex_coords);

    let world_tangent = normalize(in.world_tangent - dot(in.world_tangent, in.world_normal) * in.world_normal);
    let world_bitangent = cross(world_tangent, in.world_normal);

    // Convert to TBN [tangent->bitangent->normal] order matters.
    let TBN = mat3x3(
        world_tangent,
        world_bitangent,
        in.world_normal
    );

    let tangent_normal = object_normal.xyz * 2.0 - 1.0;
    let world_normal = TBN * tangent_normal;




    // Lighting direction vectors (all computed in Tangent Space)
    let light_dir = normalize(light.position - in.world_position);
    let view_dir = normalize(in.world_view_position - in.world_position);
    let half_dir = normalize(view_dir + light_dir); // Blinn-Phong halfway vector

    // Diffuse lighting (Lambertian)
    let diffuse_strength = max(dot(world_normal, light_dir), 0.0);
    let diffuse_color = light.color * diffuse_strength;

    // Specular lighting (Blinn-Phong)
    let specular_strength = pow(max(dot(world_normal, half_dir), 0.0), 32.0);
    let specular_color = specular_strength * light.color;

    // Reflections
    let world_reflect = reflect(-view_dir, world_normal);
    let reflection = textureSample(env_map, env_sampler, world_reflect).rgb;
    let shininess = 0.1;

    // Combine ambient, diffuse, and specular terms with diffuse albedo
    let result = (diffuse_color + specular_color) * object_color.xyz + reflection * shininess;

    return vec4f(result, object_color.a);
}
