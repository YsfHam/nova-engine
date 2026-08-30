
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

pub struct DynamicBuffer {
    buffer: wgpu::Buffer,
    length: u64,
    label: String,
}

impl DynamicBuffer {
    fn new(device: &wgpu::Device, label: &str, initial_size: u64) -> Self {
        let buffer = Self::create_buffer(device, label, initial_size);
        Self {
            buffer,
            length: 0,
            label: label.to_string()
        }
    }

    fn extend(
        &mut self, 
        data: &[u8],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder
    ) -> Offset {
        let offset = self.length;
        if data.len() as u64 + self.length <= self.buffer.size() {
            queue.write_buffer(&self.buffer, offset, data);
        }
        else {
            let new_size = self.buffer.size() * 2 + data.len() as u64 + self.length;
            let new_buffer = Self::create_buffer(device, &self.label, new_size);
            encoder.copy_buffer_to_buffer(
                &self.buffer,
                0,
                &new_buffer,
                0,
                self.length
            );

            queue.write_buffer(&new_buffer, offset, data);
            self.buffer = new_buffer;
        }

        self.length += data.len() as u64;


        Offset {
            offset,
            size: data.len() as u64
        }
    }

    fn clear(&mut self) -> &wgpu::Buffer {
        self.length = 0;
        &self.buffer
    }

    fn create_buffer(device: &wgpu::Device, label: &str, initial_size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: initial_size,
            usage: wgpu::BufferUsages::VERTEX 
                | wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }
}

pub(crate) struct StagingBufferPool {
    buffers: [DynamicBuffer; 2],
    current: usize,
}

impl StagingBufferPool {
    pub(crate) fn new(device: &wgpu::Device, initial_capacity: u64) -> Self {
        Self {
            buffers: [
                DynamicBuffer::new(device, "StagingBuffer1", initial_capacity),
                DynamicBuffer::new(device, "StagingBuffer2", initial_capacity),

            ],
            current: 0,
        }
    }

    pub(crate) fn upload(
        &mut self, 
        data: &[u8],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder
    ) -> Offset {
        self.buffers[self.current].extend(data, device, queue, encoder)
    }

    pub(crate) fn swap_buffers(&mut self) -> &wgpu::Buffer {
        let buffers_len = self.buffers.len();
        let buffer = &self.buffers[self.current].clear();
        self.current = (self.current + 1) % buffers_len;
        buffer
    }
}

pub(crate) struct Offset {
    pub(crate) offset: u64,
    pub(crate) size: u64,
}