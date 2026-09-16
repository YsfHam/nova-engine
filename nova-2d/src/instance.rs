use bytemuck::Pod;
use nova_core::math::Vec2;

/// Per-instance data for a textured/solid rectangle (quad).
///
/// Uploaded as the instance buffer for `RectangleShape` + `SpriteMaterial`.
/// The base quad's static vertices (position only) live in the shared
/// geometry buffer — uploaded once. The transform, color, and UV rect are
/// per-instance and uploaded each frame.
///
/// Layout (WGSL instance locations, offset 1):
/// - `@location(1,2,3)` transform: mat3x3 (9 floats, 36 bytes)
/// - `@location(4)` color: vec4 (4 floats, 16 bytes)
/// - `@location(5)` uv_rect: vec4 (left, top, right, bottom — 4 floats, 16 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, bytemuck::Zeroable)]
pub struct RectInstance {
    pub position: [f32; 2],
    pub scale: [f32; 2],
    pub color: [f32; 4],
    pub uv_rect: [f32; 4],
    pub rotation: f32,
}

/// Per-instance data for a filled circle.
///
/// Uploaded as the instance buffer for `CircleShape` + `CircleMaterial`.
/// The circle is inscribed in the shared quad geometry — the fragment
/// shader discards pixels outside the unit circle.
///
/// Layout (WGSL instance locations, offset 1):
/// - `@location(1)` position: vec2 (2 floats, 8 bytes)
/// - `@location(2)` radius: f32 (1 float, 4 bytes)
/// - `@location(3)` color: vec4 (4 floats, 16 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, bytemuck::Zeroable)]
pub struct CircleInstance {
    /// Center position in world space.
    pub position: [f32; 2],
    /// Circle radius in world units.
    pub radius: f32,
    /// Fill color.
    pub color: [f32; 4],
}

impl CircleInstance {
    pub fn new(position: Vec2, radius: f32, color: nova_core::graphics::color::Color) -> Self {
        Self {
            position: [position.x, position.y],
            radius,
            color: color.into(),
        }
    }
}