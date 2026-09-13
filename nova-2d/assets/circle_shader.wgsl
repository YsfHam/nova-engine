struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;

// Base vertex (from shared geometry buffer, slot 0):
//   @location(0) position: vec2 — the quad's local-space position (-0.5..0.5)
// Per-instance data (from instance buffer, slot 1):
//   @location(1) position: vec2 — circle center in world space
//   @location(2) radius: f32 — circle radius in world units
//   @location(3) color: vec4 — fill color

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) instance_position: vec2<f32>,
    @location(2) radius: f32,
    @location(3) color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;

    // Scale the quad (-0.5..0.5) to the circle's diameter, then translate
    // to the circle's world-space center.
    let world_pos = instance_position + position * (radius * 2.0);

    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);

    // Pass the local position (-0.5..0.5) for the fragment shader's
    // in/out-of-circle test.
    out.local_pos = position;
    out.color = color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // local_pos ranges from -0.5 to +0.5. The circle is inscribed in the
    // quad, so the distance from center (0,0) ranges from 0 to ~0.707.
    // Discard fragments outside the unit circle (dist > 0.5).
    let dist = length(in.local_pos);

    // Anti-aliased edge: smoothstep over a 1-pixel-wide band at the boundary.
    let edge = 0.5;
    let aa = 0.5 / fwidth(dist);
    let alpha = 1.0 - smoothstep(edge - 1.0 / max(aa, 1.0), edge, dist);

    if (alpha <= 0.0) {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}