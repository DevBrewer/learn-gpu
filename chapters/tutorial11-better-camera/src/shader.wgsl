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
    view_proj: mat4x4<f32>,
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
    @location(1) tangent_position: vec3<f32>,
    @location(2) tangent_light_position: vec3<f32>,
    @location(3) tangent_view_position: vec3<f32>,
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

    // Transform normal, tangent, and bitangent to world space and normalize
    let world_normal = normalize(normal_matrix * model.normal);
    let world_tangent = normalize(normal_matrix * model.tangent);
    let world_bitangent = normalize(normal_matrix * model.bitangent);

    // Construct the TBN matrix.
    // In WGSL, mat3x3 constructor takes columns: [T, B, N] maps Tangent Space -> World Space.
    // Transposing this orthogonal matrix gives its inverse: World Space -> Tangent Space.
    let tangent_matrix = transpose(
            mat3x3<f32>(
                world_tangent,
                world_bitangent,
                world_normal,
            ),
        );

    // Calculate vertex position in world space
    let world_position = model_matrix * vec4<f32>(model.position, 1.0);

    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    // Project position to clip space for rasterization
    out.clip_position = camera.view_proj * world_position;

    // Transform world positions into Tangent Space:
    // This allows doing Blinn-Phong lighting directly in Tangent Space in the fragment shader,
    // where the sampled normal map vector lives.
    out.tangent_position = tangent_matrix * world_position.xyz;
    out.tangent_view_position = tangent_matrix * camera.view_pos.xyz;
    out.tangent_light_position = tangent_matrix * light.position;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample base diffuse texture (albedo)
    let object_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);

    // Sample normal map: colors [0.0, 1.0] are mapped to tangent-space vector [-1.0, 1.0]
    let object_normal = textureSample(t_normal, s_normal, in.tex_coords);
    let tangent_normal = object_normal.xyz * 2.0 - 1.0;

    // 1. Ambient lighting
    let ambient_strength = 0.1;
    let ambient_color = light.color * ambient_strength;

    // 2. Lighting direction vectors (all computed in Tangent Space)
    let light_dir = normalize(in.tangent_light_position - in.tangent_position);
    let view_dir = normalize(in.tangent_view_position - in.tangent_position);
    let half_dir = normalize(view_dir + light_dir); // Blinn-Phong halfway vector

    // 3. Diffuse lighting (Lambertian)
    let diffuse_strength = max(dot(tangent_normal, light_dir), 0.0);
    let diffuse_color = light.color * diffuse_strength;

    // 4. Specular lighting (Blinn-Phong)
    let specular_strength = pow(max(dot(tangent_normal, half_dir), 0.0), 32.0);
    let specular_color = specular_strength * light.color;

    // Combine ambient, diffuse, and specular terms with diffuse albedo
    let result = (ambient_color + diffuse_color + specular_color) * object_color.xyz;

    return vec4f(result, object_color.a);
}
