// This is a separate file that we'll write to disk at runtime
// src/shader_code.wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) instance_position: vec2<f32>,
    @location(2) instance_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Scale the particle to the correct size
    let particle_pos = vertex.position * PARTICLE_SIZE;

    // Convert the instance position from pixel coordinates to clip space
    let x = (vertex.instance_position.x + particle_pos.x) / SCREEN_WIDTH * 2.0 - 1.0;
    let y = -((vertex.instance_position.y + particle_pos.y) / SCREEN_HEIGHT * 2.0 - 1.0);

    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.color = vertex.instance_color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

// Constants injected by the renderer
var<private> PARTICLE_SIZE: f32 = 3.0;
var<private> SCREEN_WIDTH: f32 = 800.0;
var<private> SCREEN_HEIGHT: f32 = 600.0;


