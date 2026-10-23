// WGSL Shader for Donuts Vertex / Multiple Instance Buffers

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) per_vertex_color: vec4<f32>,
}

struct StaticInstanceInput {
    @location(2) color: vec4<f32>,
    @location(3) offset: vec2<f32>,
}

struct ChangingInstanceInput {
    @location(4) scale: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(
    vert: VertexInput,
    static_inst: StaticInstanceInput,
    changing_inst: ChangingInstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Apply scale and offset transformation per instance
    let scaled_pos = vert.position * changing_inst.scale + static_inst.offset;
    out.position = vec4<f32>(scaled_pos, 0.0, 1.0);

    // Modulate per-vertex gradient with per-instance color
    out.color = static_inst.color * vert.per_vertex_color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
