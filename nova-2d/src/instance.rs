use bytemuck::Pod;
use nova_core::math::Mat3;

/// Per-instance data for a 2D quad. Uploaded as instance buffer (step mode
/// `Instance`). The base quad's static vertices (position only) live in the
/// shared geometry buffer — uploaded once. The transform, color, and UV rect
/// are per-instance and uploaded each frame.
///
/// Layout (WGSL locations 0–2 on the instance buffer, offset from
/// `InstanceBufferLayout::new(..., 0)`):
/// - `@location(0)` transform: mat3x3 (9 floats, 36 bytes)
/// - `@location(1)` color: vec4 (4 floats, 16 bytes)
/// - `@location(2)` uv_rect: vec4 (left, top, right, bottom — 4 floats, 16 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, bytemuck::Zeroable)]
pub struct InstanceData2D {
    /// 2D transform matrix (scale × rotation × translation).
    pub transform: Mat3,
    /// Per-quad color (multiplied with texture sample).
    pub color: [f32; 4],
    /// UV rectangle: (left, top, right, bottom).
    pub uv_rect: [f32; 4],
}

impl InstanceData2D {
    pub fn new(transform: Mat3, color: nova_core::graphics::color::Color, uv_rect: crate::utils::RectF32) -> Self {
        Self {
            transform,
            color: color.into(),
            uv_rect: [uv_rect.left, uv_rect.top, uv_rect.right, uv_rect.bottom],
        }
    }
}