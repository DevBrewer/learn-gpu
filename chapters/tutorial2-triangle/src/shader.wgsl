struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    let positions = array<vec3<f32>, 3>(
            vec3<f32>(0.0, 0.5, 0.0),
            vec3<f32>(-0.5, -0.5, 0.0),
            vec3<f32>(0.5, -0.5, 0.0),
        );
    let colors = array<vec3<f32>, 3>(
            vec3<f32>(1.0, 0.0, 0.0),
            vec3<f32>(0.0, 1.0, 0.0),
            vec3<f32>(0.0, 0.0, 1.0),
        );

    out.clip_position = vec4<f32>(positions[in_vertex_index], 1.0);
    out.color = vec4<f32>(colors[in_vertex_index], 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
