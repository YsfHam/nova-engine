#[derive(Clone, Debug)]
pub struct VertexBufferLayout {
    attributes: Vec<wgpu::VertexAttribute>,
    vertex_stride: u64,
}

/// Engine-native vertex attribute format. A serializable mirror of the
/// subset of `wgpu::VertexFormat` we expose. Translate to `wgpu` via the
/// `From` impl below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexFormat {
    Float32,
    Float32x2,
    Float32x3,
    Float32x4,
    Uint8x4,
    Uint16x2,
    Uint32,
}

impl VertexFormat {
    /// Byte size of one value of this format — used to compute vertex stride.
    pub fn size(&self) -> u64 {
        match self {
            VertexFormat::Float32 => 4,
            VertexFormat::Float32x2 => 8,
            VertexFormat::Float32x3 => 12,
            VertexFormat::Float32x4 => 16,
            VertexFormat::Uint8x4 => 4,
            VertexFormat::Uint16x2 => 4,
            VertexFormat::Uint32 => 4,
        }
    }
}

impl From<VertexFormat> for wgpu::VertexFormat {
    fn from(f: VertexFormat) -> Self {
        match f {
            VertexFormat::Float32 => wgpu::VertexFormat::Float32,
            VertexFormat::Float32x2 => wgpu::VertexFormat::Float32x2,
            VertexFormat::Float32x3 => wgpu::VertexFormat::Float32x3,
            VertexFormat::Float32x4 => wgpu::VertexFormat::Float32x4,
            VertexFormat::Uint8x4 => wgpu::VertexFormat::Uint8x4,
            VertexFormat::Uint16x2 => wgpu::VertexFormat::Uint16x2,
            VertexFormat::Uint32 => wgpu::VertexFormat::Uint32,
        }
    }
}

impl VertexBufferLayout {

    pub fn empty() -> Self {
        Self::new(&[])
    }

    /// Builds a layout from an ordered list of attribute formats. Each
    /// attribute is assigned a sequential `shader_location` (0, 1, 2, ...),
    /// and the stride is the sum of the attribute sizes.
    ///
    pub fn new(attributes_formats: &[VertexFormat]) -> Self {
        let mut offset = 0;
        let mut shader_location = 0;
        let mut attributes = vec![];

        for format in attributes_formats {
            attributes.push(wgpu::VertexAttribute {
                format: (*format).into(),
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

    /// Whether this layout declares any vertex attributes. An empty layout
    /// (`new(&[])`) means the pipeline has no vertex buffer — vertex data is
    /// generated in the shader (e.g. via `vertex_index`).
    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
    }
}

impl<'a> TryInto<wgpu::VertexBufferLayout<'a>> for &'a VertexBufferLayout {
    type Error = ();

    fn try_into(self) -> Result<wgpu::VertexBufferLayout<'a>, Self::Error> {
        if self.is_empty() {
            Err(())
        }
        else {
            Ok(wgpu::VertexBufferLayout {
                array_stride: self.vertex_stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &self.attributes
            })
        }
    }
}

