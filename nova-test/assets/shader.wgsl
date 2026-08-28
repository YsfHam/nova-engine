// Quad shader — colored quad with vertex buffer.
//
// Vertex data comes from a vertex buffer (position: vec2<f32>). The fragment
// color comes from a material uniform in group 1, binding 0 (a vec4<f32>).

// Group 0 (scene): binding 0 = camera (mat4), binding 1 = time (f32).
// (Declared but unused for this 2D quad — the quad fills the screen.)
struct CameraUniform {
    view_proj: mat4x4<f32>,
};
struct SceneUniform {
    time: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> scene: SceneUniform;

// Group 1 (material): binding 0 = color (vec4).
struct MaterialUniform {
    color: vec4<f32>,
};
@group(1) @binding(0) var<uniform> material: MaterialUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return material.color;
}
