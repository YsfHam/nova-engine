struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) col0: vec3<f32>,
    @location(2) col1: vec3<f32>,
    @location(3) col2: vec3<f32>,
    @location(4) color: vec4<f32>,
    @location(5) uv_rect: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let transform = mat3x3<f32>(col0, col1, col2);
    let transformed = transform * vec3<f32>(position, 1.0);
    out.clip_position = camera.view_proj * vec4<f32>(transformed.xy, 0.0, 1.0);
    out.color = color;
    return out;
}

@group(1) @binding(0) var<uniform> color_uniform: vec4<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return color_uniform * in.color;
}