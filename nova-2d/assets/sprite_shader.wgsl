
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;


struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct Instance {
    @location(1) position: vec2<f32>,
    @location(2) scale: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) uv_rect: vec4<f32>,
    @location(5) rotation: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

fn make_transform(position: vec2<f32>, scale: vec2<f32>, rotation: f32) -> mat3x3<f32> {
    let c = cos(rotation);
    let s = sin(rotation);
    return mat3x3<f32>(
        vec3(c * scale.x, s * scale.x, 0.0),
        vec3(-s * scale.y, c * scale.y, 0.0),
        vec3(position.x, position.y, 1.0),
    );
}

@vertex
fn vs_main(
   vertex_data: VertexInput,
   instance_data: Instance
) -> VertexOutput {
    var out: VertexOutput;

    let transform = make_transform(instance_data.position, instance_data.scale, instance_data.rotation);

    // Apply the per-instance 2D transform to the base position.
    let transformed = transform * vec3<f32>(vertex_data.position, 1.0);

    // Apply the camera view-projection to the transformed position.
    out.clip_position = camera.view_proj * vec4<f32>(transformed.xy, 0.0, 1.0);

    // Map the base UV (0..1) to the instance's UV rect.
    // Base vertex positions are TL(-0.5,-0.5), BL(-0.5,0.5), BR(0.5,0.5), TR(0.5,-0.5).
    // Map: u = position.x + 0.5, v = position.y + 0.5 → 0..1
    let uv_rect = instance_data.uv_rect;
    let base_u = vertex_data.position.x + 0.5;
    let base_v = vertex_data.position.y + 0.5;
    out.uv = vec2<f32>(
        uv_rect.x + (uv_rect.z - uv_rect.x) * base_u,  // left + (right-left)*u
        uv_rect.y + (uv_rect.w - uv_rect.y) * base_v,  // top + (bottom-top)*v
    );

    out.color = instance_data.color;
    return out;
}

@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var tex_sampler: sampler;


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, in.uv) * in.color;
}
