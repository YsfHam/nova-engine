use std::num::NonZeroU32;

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
#[derive(Clone, Debug)]
pub struct DrawBatch {
    
    pub material: Handle<Material>,

    pub vertices: Vec<u8>,
    
    pub indices: Vec<u16>,

    pub instances: Option<Vec<u8>>,

    instance_stride: u32,
}

impl DrawBatch {

    pub fn with_vertices<V: NoUninit>(
        material: Handle<Material>,
        vertices: &[V],
        indices: Vec<u16>,
    ) -> Self {
        Self {
            material,
            vertices: bytemuck::cast_slice(vertices).to_vec(),
            indices,
            instances: None,
            instance_stride: 1,
        }
    }

    /// Builder: add instance data (enables instanced drawing).
    pub fn with_instances(mut self, instances: Vec<u8>, instance_stride: NonZeroU32) -> Self {
        self.instances = Some(instances);
        self.instance_stride = instance_stride.get();
        self
    }

    /// The number of indices in this batch (each u16 index references one
    /// vertex).
    pub fn index_count(&self) -> u32 {
        self.indices.len() as u32
    }

    pub fn instance_count(&self) -> u32 {
        self.instances
        .as_ref()
        .map(|instances| instances.len() as u32 / self.instance_stride)
        .unwrap_or(1)
    }
}

pub struct VertexBatch {
    vertices: Vec<u8>,
    vertex_stride: u32,
    indices: Vec<u16>,
}

impl VertexBatch {
    pub fn new(vertex_stride: u32) -> Self {
        Self {
            vertices: vec![],
            vertex_stride,
            indices: vec![]
        }
    }

    pub fn add_vertices<V: NoUninit>(&mut self, vertices: &[V], indices: &[u16]) {
        let offset = self.vertex_count();
        self.vertices.extend_from_slice(bytemuck::cast_slice(vertices));
        let offseted_indices =  
            indices.iter()
            .map(|index| index + offset as u16)
            .collect::<Vec<_>>()
        ;
        self.indices.extend_from_slice(&offseted_indices);
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertices.len() as u32 / self.vertex_stride
    }

    pub fn vertices(&self) -> &[u8] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u16] {
        &self.indices
    }
}


pub struct InstanceBatch {
    instances: Vec<u8>,
    instance_stride: u32,
}

impl InstanceBatch {
    pub fn new(instance_stride: u32) -> Self {
        Self {
            instance_stride,
            instances: vec![]
        }
    }

    pub fn add_instances<I: NoUninit>(&mut self, instances: &[I]) {
        let type_size = std::mem::size_of::<I>();
        if type_size != self.instance_stride as usize {
            panic!("Instance size mismatch epxected: {}, found {}", self.instance_stride, type_size);
        }

        self.instances.extend_from_slice(bytemuck::cast_slice(instances));
    }
}