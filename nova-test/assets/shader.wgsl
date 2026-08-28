// Quad shader — colored quad generated in the vertex shader.
//
// No vertex buffer: the quad's two triangles (6 vertices) are produced from
// `vertex_index`. The fragment color comes from a material uniform in
// group 1, binding 0 (a `vec4<f32>`).

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

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

// Two triangles covering a centered quad in clip space, selected by
// vertex_index. No vertex buffer needed.
//
//  vertex_index:  0   1   2   3   4   5
//  position:     TL  TR  BR  TL  BR  BL
//
// with x,y in [-0.5, 0.5].
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-0.5,  0.5),  // TL
        vec2<f32>(-0.5, -0.5),  // BL
        vec2<f32>( 0.5, -0.5),  // BR
        vec2<f32>( 0.5,  0.5),  // TR
        vec2<f32>(-0.5,  0.5),  // TL
        vec2<f32>( 0.5, -0.5),  // BR
    );

    var out: VertexOutput;
    let p = positions[vi];
    out.clip_position = vec4<f32>(p, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return material.color;
}
