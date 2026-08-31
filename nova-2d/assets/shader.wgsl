
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;


// Base vertex (from shared geometry buffer, slot 0):
//   @location(0) position: vec2 — the quad's local-space position (-0.5..0.5)
// Per-instance data (from instance buffer, slot 1):
//   @location(1,2,3) transform: mat3x3 — per-quad 2D transform (3 columns × vec3)
//   @location(4) color: vec4 — per-quad color
//   @location(5) uv_rect: vec4 — (left, top, right, bottom)

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
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

    // Reconstruct the 3x3 transform matrix from its 3 columns.
    let transform = mat3x3<f32>(col0, col1, col2);

    // Apply the per-instance 2D transform to the base position.
    let transformed = transform * vec3<f32>(position, 1.0);

    // Apply the camera view-projection to the transformed position.
    out.clip_position = camera.view_proj * vec4<f32>(transformed.xy, 0.0, 1.0);

    // Map the base UV (0..1) to the instance's UV rect.
    // Base vertex positions are TL(-0.5,-0.5), BL(-0.5,0.5), BR(0.5,0.5), TR(0.5,-0.5).
    // Map: u = position.x + 0.5, v = position.y + 0.5 → 0..1
    let base_u = position.x + 0.5;
    let base_v = position.y + 0.5;
    out.uv = vec2<f32>(
        uv_rect.x + (uv_rect.z - uv_rect.x) * base_u,  // left + (right-left)*u
        uv_rect.y + (uv_rect.w - uv_rect.y) * base_v,  // top + (bottom-top)*v
    );

    out.color = color;
    return out;
}

@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var tex_sampler: sampler;


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, in.uv) * in.color;
}
