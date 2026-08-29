#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex2D {
    pub position: [f32; 4],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

