// ---------------------------------------------------------------------------
// Image Editor Post-Processing & Color Grading Shader
// ---------------------------------------------------------------------------

struct EditorUniform {
    transform: vec4<f32>,     // scale_x, scale_y, pan_x, pan_y
    brightness: f32,         // -1.0 to 1.0
    contrast: f32,           // 0.0 to 2.0
    saturation: f32,         // 0.0 to 2.0
    exposure: f32,           // 0.1 to 3.0
    hue: f32,                // -PI to PI
    warmth: f32,             // -1.0 to 1.0
    vignette: f32,           // 0.0 to 1.0
    split_pos: f32,          // -1.0 (disabled) or 0.0..1.0
    invert: u32,             // 0 or 1
    grayscale: u32,          // 0 or 1
    sepia: u32,              // 0 or 1
    aspect_ratio: f32,       // (img_w / img_h) / (screen_w / screen_h)
}

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> uniforms: EditorUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) screen_uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) id: u32) -> VertexOutput {
    // 2 Triangles forming a unit quad [-1, 1]
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    let pos = positions[id];
    var out: VertexOutput;

    // Apply scale, aspect ratio correction, and pan translation
    let scaled_pos = vec2<f32>(
        pos.x * uniforms.transform.x * uniforms.aspect_ratio + uniforms.transform.z,
        pos.y * uniforms.transform.y + uniforms.transform.w,
    );

    out.position = vec4<f32>(scaled_pos, 0.0, 1.0);
    // Texture coordinates in [0, 1] with Y flipped for top-down image layout
    out.uv = vec2<f32>(pos.x * 0.5 + 0.5, 0.5 - pos.y * 0.5);
    out.screen_uv = vec2<f32>(scaled_pos.x * 0.5 + 0.5, 0.5 - scaled_pos.y * 0.5);
    return out;
}

// Convert RGB to HSV color space
fn rgb_to_hsv(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4<f32>(c.bg, K.wz), vec4<f32>(c.gb, K.xy), step(c.b, c.g));
    let q = mix(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), step(p.x, c.r));
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

// Convert HSV back to RGB color space
fn hsv_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Discard any fragments outside the bounds of the image quad
    if (in.uv.x < 0.0 || in.uv.x > 1.0 || in.uv.y < 0.0 || in.uv.y > 1.0) {
        discard;
    }

    let orig = textureSample(t_diffuse, s_diffuse, in.uv);

    // A/B Split Screen logic:
    // When split_pos >= 0.0, show unadjusted original to the left of the split divider
    if (uniforms.split_pos >= 0.0) {
        let dist = abs(in.screen_uv.x - uniforms.split_pos);
        if (dist < 0.002) {
            // White divider line
            return vec4<f32>(1.0, 1.0, 1.0, 1.0);
        }
        if (in.screen_uv.x < uniforms.split_pos) {
            return orig;
        }
    }

    var color = orig.rgb;

    // 1. Exposure adjustment (exponential scaling)
    color = color * pow(2.0, uniforms.exposure - 1.0);

    // 2. Brightness adjustment
    color = color + vec3<f32>(uniforms.brightness);

    // 3. Contrast adjustment centered around middle gray (0.5)
    color = (color - 0.5) * uniforms.contrast + 0.5;

    // 4. Hue Rotation
    if (abs(uniforms.hue) > 0.001) {
        var hsv = rgb_to_hsv(color);
        hsv.x = fract(hsv.x + uniforms.hue / (2.0 * 3.14159265));
        color = hsv_to_rgb(hsv);
    }

    // 5. Saturation
    let luma = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    color = mix(vec3<f32>(luma), color, uniforms.saturation);

    // 6. Color Temperature / Warmth
    if (abs(uniforms.warmth) > 0.001) {
        color.r = color.r + uniforms.warmth * 0.15;
        color.b = color.b - uniforms.warmth * 0.15;
    }

    // 7. Sepia filter
    if (uniforms.sepia == 1u) {
        let r = dot(color, vec3<f32>(0.393, 0.769, 0.189));
        let g = dot(color, vec3<f32>(0.349, 0.686, 0.168));
        let b = dot(color, vec3<f32>(0.272, 0.534, 0.131));
        color = vec3<f32>(r, g, b);
    }

    // 8. Grayscale
    if (uniforms.grayscale == 1u) {
        let g = dot(color, vec3<f32>(0.299, 0.587, 0.114));
        color = vec3<f32>(g);
    }

    // 9. Invert Colors
    if (uniforms.invert == 1u) {
        color = vec3<f32>(1.0) - color;
    }

    // 10. Vignette effect around the edges
    if (uniforms.vignette > 0.001) {
        let center_offset = in.uv - vec2<f32>(0.5, 0.5);
        let dist = length(center_offset) * 1.4142;
        let vig = 1.0 - smoothstep(0.4, 1.0, dist) * uniforms.vignette;
        color = color * vig;
    }

    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(color, orig.a);
}
