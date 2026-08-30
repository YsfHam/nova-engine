
use bytemuck::NoUninit;

use crate::{
    assets::handle::Handle,
    graphics::material::Material,
};

/// A single draw batch: a material reference + raw geometry data.
///
/// `DrawBatch` is **pure data** — it holds no GPU resources (`wgpu::Buffer`,
/// etc.). Vertex, index, and optional instance data are raw byte slices that
/// the [`RenderTargetCommander`](super::render_target::RenderTargetCommander)
/// uploads to GPU buffers at submit time. This makes `DrawBatch` cheap to
/// create, clone, collect, and sort — the batcher can accumulate thousands
/// without touching the GPU.
///
/// The material is referenced by [`Handle<Material>`] (unresolved) — the
/// commander resolves it to GPU resources (pipeline, bind group) at draw time
/// via the `AssetsManager`.
///
/// # Submission order
///
/// Batches are drawn in the exact order given to
/// [`submit_batches`](super::render_target::RenderTargetCommander::submit_batches).
/// The commander does **no sorting or grouping** — it trusts the caller to
/// order batches correctly (e.g. by layer, grouped by template for pipeline
/// reuse). Each batch gets its own vertex/index buffer upload and draw call.
///
/// # Index format
///
/// Indices are always `u16` (`Uint16`). This is sufficient for up to 65K
/// vertices per batch — more than enough for 2D quads and most 3D meshes.
#[derive(Debug)]
pub struct DrawBatch {
    
    pub material: Handle<Material>,

    vertex_batch: VertexBatch,

    instance_batch: Option<InstanceBatch>,
}

impl DrawBatch {

    pub fn new(material: Handle<Material>, vertex_stride: u32) -> Self {
        Self {
            material,
            vertex_batch: VertexBatch::new(vertex_stride),
            instance_batch: None,
        }
    }

    pub fn with_vertices<V: NoUninit>(
        material: Handle<Material>,
        vertices: &[V],
        vertex_stride: u32,
        indices: &[u16],
    ) -> Self {

        let mut slf = Self::new(material, vertex_stride);
        slf.add_vertices(vertices, indices);

        slf
    }

    /// Builder: add instance data (enables instanced drawing).
    pub fn with_instances<I: NoUninit>(mut self, instances: &[I], instance_stride: u32) -> Self {
        let mut instance_batch = InstanceBatch::new(instance_stride);
        instance_batch.add_instances(instances);
        self.instance_batch = Some(instance_batch);
        self
    }

    pub fn add_instances<I: NoUninit>(&mut self, instances: &[I]) {
        if let Some(instance_batch) = self.instance_batch.as_mut() {
            instance_batch.add_instances(instances);
        }
    }

    pub fn add_vertices<V: NoUninit>(&mut self, vertices: &[V], indices: &[u16]) {
        self.vertex_batch.add_vertices(vertices, indices);
    }

    pub fn index_count(&self) -> u32 {
        self.vertex_batch.index_count()
    }

    pub fn vertices(&self) -> &[u8] {
        &self.vertex_batch.vertices
    }

    pub fn indices(&self) -> &[u16] {
        &self.vertex_batch.indices
    }

    pub fn instances(&self) -> Option<&[u8]> {
        self
            .instance_batch
            .as_ref()
            .map(|instance_batch| instance_batch.instances.as_slice())
    }

    pub fn instance_count(&self) -> u32 {
        self.instance_batch
        .as_ref()
        .map(|instance_batch| instance_batch.len())
        .unwrap_or(1)
    }
}

#[derive(Debug)]
struct VertexBatch {
    vertices: Vec<u8>,
    vertex_stride: u32,
    indices: Vec<u16>,
}

impl VertexBatch {
    fn new(vertex_stride: u32) -> Self {
        Self {
            vertices: vec![],
            vertex_stride,
            indices: vec![]
        }
    }

    fn add_vertices<V: NoUninit>(&mut self, vertices: &[V], indices: &[u16]) {
        let offset = self.vertex_count();
        self.vertices.extend_from_slice(bytemuck::cast_slice(vertices));
        let offseted_indices =  
            indices.iter()
            .map(|index| index + offset as u16)
            .collect::<Vec<_>>()
        ;
        self.indices.extend_from_slice(&offseted_indices);
    }

    fn vertex_count(&self) -> u32 {
        self.vertices.len() as u32 / self.vertex_stride
    }

    fn index_count(&self) -> u32 {
        self.indices.len() as u32
    }
}


#[derive(Debug)]
struct InstanceBatch {
    instances: Vec<u8>,
    instance_stride: u32,
}

impl InstanceBatch {
    fn new(instance_stride: u32) -> Self {
        Self {
            instance_stride,
            instances: vec![]
        }
    }

    fn add_instances<I: NoUninit>(&mut self, instances: &[I]) {
        let type_size = std::mem::size_of::<I>();
        if type_size != self.instance_stride as usize {
            panic!("Instance size mismatch epxected: {}, found {}", self.instance_stride, type_size);
        }

        self.instances.extend_from_slice(bytemuck::cast_slice(instances));
    }

    fn len(&self) -> u32 {
        self.instances.len() as u32 / self.instance_stride
    }
}