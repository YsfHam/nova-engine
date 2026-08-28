#[derive(Clone, Debug)]
pub struct BufferLayout {
    attributes: Vec<wgpu::VertexAttribute>,
    stride: u64,
    step_mode: BufferStepMode,
}

pub struct VertexBufferLayout {
    _private: ()
}

impl VertexBufferLayout {
    pub fn new(attributes_formats: &[VertexFormat], location_offset: u32) -> BufferLayout {
        BufferLayout::new(attributes_formats, BufferStepMode::Vertex, location_offset)
    }
}

pub struct InstanceBufferLayout {
    _private: ()
}

impl InstanceBufferLayout {
    pub fn new(attributes_formats: &[VertexFormat], location_offset: u32) -> BufferLayout {
        BufferLayout::new(attributes_formats, BufferStepMode::Instance, location_offset)
    }
}


/// Whether a [`BufferLayout`] describes per-vertex data or per-instance data.
/// Mirrors `wgpu::VertexStepMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferStepMode {
    /// The buffer is advanced per vertex (the usual vertex buffer).
    Vertex,
    /// The buffer is advanced per instance (instanced attribute buffer).
    Instance,
}

impl From<BufferStepMode> for wgpu::VertexStepMode {
    fn from(m: BufferStepMode) -> Self {
        match m {
            BufferStepMode::Vertex => wgpu::VertexStepMode::Vertex,
            BufferStepMode::Instance => wgpu::VertexStepMode::Instance,
        }
    }
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

impl BufferLayout {
    pub fn empty() -> Self {
        Self::new(&[], BufferStepMode::Vertex, 0)
    }

    /// Builds a vertex buffer layout (per-vertex step mode) from an ordered
    /// list of attribute formats. Each attribute is assigned a sequential
    /// `shader_location` (0, 1, 2, ...), and the stride is the sum of the
    /// attribute sizes.
    fn new(attributes_formats: &[VertexFormat], step_mode: BufferStepMode, location_offset: u32) -> Self {
        let mut offset = 0;
        let mut shader_location = location_offset;
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
            stride: offset,
            step_mode,
        }
    }

    /// Whether this layout declares any vertex attributes. An empty layout
    /// (`new(&[])`) means the pipeline has no vertex buffer — vertex data is
    /// generated in the shader (e.g. via `vertex_index`).
    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
    }

    /// The byte stride between consecutive elements (vertices or instances)
    /// in a buffer using this layout. This is the sum of all attribute sizes.
    pub fn stride(&self) -> u64 {
        self.stride
    }

    /// The step mode of this layout (per-vertex or per-instance).
    pub fn step_mode(&self) -> BufferStepMode {
        self.step_mode
    }
}

impl<'a> TryInto<wgpu::VertexBufferLayout<'a>> for &'a BufferLayout {
    type Error = ();

    fn try_into(self) -> Result<wgpu::VertexBufferLayout<'a>, Self::Error> {
        if self.is_empty() {
            Err(())
        }
        else {
            Ok(wgpu::VertexBufferLayout {
                array_stride: self.stride,
                step_mode: self.step_mode.into(),
                attributes: &self.attributes
            })
        }
    }
}

