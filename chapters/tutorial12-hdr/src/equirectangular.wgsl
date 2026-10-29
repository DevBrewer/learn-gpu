// Computes equirectangular spherical panorama to cubemap conversion.
// Each cube face (+X, -X, +Y, -Y, +Z, -Z) is populated on the GPU via compute shader.
const PI: f32 = 3.1415926535897932384626433832795;

// Face orientation vectors used to map 2D face UVs to 3D direction vectors
struct Face {
    forward: vec3<f32>,
    up: vec3<f32>,
    right: vec3<f32>,
}

// Group 0, Binding 0: 2D equirectangular input texture (sampled via textureLoad)
@group(0)
@binding(0)
var src: texture_2d<f32>;

// Group 0, Binding 1: Output 2D array storage texture with 6 layers for each cube face
@group(0)
@binding(1)
var dst: texture_storage_2d_array<rgba32float, write>;

// Compute kernel entry point:
// Dispatched with workgroup size (16, 16, 1).
// Dimensions: (dst_size/16, dst_size/16, 6)
@compute
@workgroup_size(16, 16, 1)
fn compute_equirect_to_cubemap(
    @builtin(global_invocation_id)
    gid: vec3<u32>,
) {
    // Bounds check to ensure we do not write past destination texture boundary
    if gid.x >= u32(textureDimensions(dst).x) {
        return;
    }

    // Coordinate bases for the 6 cube map faces:
    // +X (0), -X (1), +Y (2), -Y (3), +Z (4), -Z (5)
    var FACES: array<Face, 6> = array(
        // Face +X
        Face(
            vec3(1.0, 0.0, 0.0),  // forward
            vec3(0.0, 1.0, 0.0),  // up
            vec3(0.0, 0.0, -1.0), // right
        ),
        // Face -X
        Face (
            vec3(-1.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            vec3(0.0, 0.0, 1.0),
        ),
        // Face +Y
        Face (
            vec3(0.0, 1.0, 0.0),
            vec3(0.0, 0.0, 1.0),
            vec3(1.0, 0.0, 0.0),
        ),
        // Face -Y
        Face (
            vec3(0.0, -1.0, 0.0),
            vec3(0.0, 0.0, -1.0),
            vec3(1.0, 0.0, 0.0),
        ),
        // Face +Z
        Face (
            vec3(0.0, 0.0, 1.0),
            vec3(0.0, 1.0, 0.0),
            vec3(1.0, 0.0, 0.0),
        ),
        // Face -Z
        Face (
            vec3(0.0, 0.0, -1.0),
            vec3(0.0, 1.0, 0.0),
            vec3(-1.0, 0.0, 0.0),
        ),
    );

    // Get normalized coordinates [-1.0, 1.0] relative to cubemap face center
    let dst_dimensions = vec2<f32>(textureDimensions(dst));
    let cube_uv = vec2<f32>(gid.xy) / dst_dimensions * 2.0 - 1.0;

    // Convert face local 2D UV into a 3D unit ray pointing outward from the cube center
    let face = FACES[gid.z];
    let spherical = normalize(face.forward + face.right * cube_uv.x + face.up * cube_uv.y);

    // Convert 3D Cartesian coordinates to spherical coordinates (longitude/latitude)
    // and map to normalized equirectangular UV [0.0, 1.0]
    let inv_atan = vec2(0.1591, 0.3183); // 1/(2*PI), 1/PI
    let eq_uv = vec2(atan2(spherical.z, spherical.x), asin(spherical.y)) * inv_atan + 0.5;
    let eq_pixel = vec2<i32>(eq_uv * vec2<f32>(textureDimensions(src)));

    // Fetch HDR texel using textureLoad (textureSample is not available in compute shaders)
    var sample = textureLoad(src, eq_pixel, 0);

    // Store into destination cubemap layer (gid.z = face index 0..5)
    textureStore(dst, gid.xy, gid.z, sample);
}

