#[derive(Clone, Debug)]
pub struct VertexBufferLayout {
    attributes: Vec<wgpu::VertexAttribute>,
    vertex_stride: u64,
}

impl VertexBufferLayout {
    pub fn new(attributes_formats: &[wgpu::VertexFormat]) -> Self {
        let mut offset = 0;
        let mut shader_location = 0;
        let mut attributes = vec![];

        for format in attributes_formats {
            attributes.push(wgpu::VertexAttribute {
                format: *format,
                offset,
                shader_location,
            });
            shader_location += 1;
            offset += format.size()
        }

        Self {
            attributes,
            vertex_stride: offset,
        }
    }
}

impl<'a> Into<wgpu::VertexBufferLayout<'a>> for &'a VertexBufferLayout {
    fn into(self) -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: self.vertex_stride,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &self.attributes
        }
    }
}

