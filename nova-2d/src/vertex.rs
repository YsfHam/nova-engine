/// Static base vertex for a 2D quad. Only position — uploaded once to the
/// shared geometry buffer. The transform, color, and UV rect come from the
/// per-instance data (`InstanceData2D`).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BaseVertex2D {
    pub position: [f32; 2],
}

/// Legacy per-vertex data (used when not instancing). Kept for reference;
/// the instancing path uses `BaseVertex2D` + `InstanceData2D` instead.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex2D {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

