
use bytemuck::NoUninit;

use crate::{
    assets::handle::Handle,
    graphics::{geometry::GeometryRef, material::Material},
};

/// A single draw batch: a material reference + geometry (owned or shared).
///
/// `DrawBatch` is **pure data** — it holds no GPU resources. Geometry is
/// either **owned** (per-frame vertex/index data uploaded and discarded each
/// frame) or **shared** (a [`GeometryRef`] pointing to persistent geometry in
/// the [`GeometryPool`](crate::graphics::geometry::GeometryPool), uploaded
/// once and reused across frames and batches).
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
/// reuse). Each batch gets its own draw call.
///
/// # Index format
///
/// Indices are always `u16` (`Uint16`). This is sufficient for up to 65K
/// vertices per batch — more than enough for 2D quads and most 3D meshes.
#[derive(Debug)]
pub struct DrawBatch {
    pub material: Handle<Material>,
    geometry: BatchGeometry,
    instance_batch: Option<InstanceBatch>,
}

/// The geometry source for a `DrawBatch`.
#[derive(Debug)]
pub enum BatchGeometry {
    /// Owned vertex/index data — uploaded to the per-frame staging buffer
    /// each frame and discarded.
    Owned(VertexBatch),
    /// Reference to shared, persistent geometry — uploaded once to the
    /// `GeometryPool` buffer, reused across frames and batches. No per-frame
    /// upload needed.
    Shared(GeometryRef),
}

impl DrawBatch {
    /// Creates a batch with **owned** geometry (empty, to be filled via
    /// [`add_vertices`](Self::add_vertices)).
    pub fn new(material: Handle<Material>, vertex_stride: u32) -> Self {
        Self {
            material,
            geometry: BatchGeometry::Owned(VertexBatch::new(vertex_stride)),
            instance_batch: None,
        }
    }

    /// Creates a batch with **owned** geometry from pre-built vertices + indices.
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

    /// Creates a batch referencing **shared** geometry from a [`GeometryRef`].
    /// The geometry must have been inserted into the `GeometryPool` first.
    /// No per-frame upload — the offsets are permanent.
    pub fn with_shared_geometry(material: Handle<Material>, geometry: GeometryRef) -> Self {
        Self {
            material,
            geometry: BatchGeometry::Shared(geometry),
            instance_batch: None,
        }
    }

    /// Builder: add instance data (enables instanced drawing).
    pub fn with_instances<I: NoUninit>(mut self, instances: &[I], instance_stride: u32) -> Self {
        let mut instance_batch = InstanceBatch::new(instance_stride);
        instance_batch.add_instances(instances);
        self.instance_batch = Some(instance_batch);
        self
    }

    pub fn add_instances<I: NoUninit>(&mut self, instances: &[I]) {
        let instance_stride = std::mem::size_of::<I>() as u32;
        if self.instance_batch.is_none() {
            self.instance_batch = Some(InstanceBatch::new(instance_stride));
        }
        if let Some(instance_batch) = self.instance_batch.as_mut() {
            instance_batch.add_instances(instances);
        }
    }

    /// Adds vertices to the batch. Only valid when the geometry is **owned**
    /// (not shared). Panics if the geometry is shared.
    pub fn add_vertices<V: NoUninit>(&mut self, vertices: &[V], indices: &[u16]) {
        match &mut self.geometry {
            BatchGeometry::Owned(vb) => vb.add_vertices(vertices, indices),
            BatchGeometry::Shared(_) => {
                panic!("Cannot add_vertices to a DrawBatch with shared geometry");
            }
        }
    }

    /// Returns the vertex data slice. Only valid for owned geometry.
    /// Returns `None` for shared geometry (the data lives in the GeometryPool).
    pub fn vertices(&self) -> Option<&[u8]> {
        match &self.geometry {
            BatchGeometry::Owned(vb) => Some(&vb.vertices),
            BatchGeometry::Shared(_) => None,
        }
    }

    /// Returns the index data slice. Only valid for owned geometry.
    /// Returns `None` for shared geometry.
    pub fn indices(&self) -> Option<&[u16]> {
        match &self.geometry {
            BatchGeometry::Owned(vb) => Some(&vb.indices),
            BatchGeometry::Shared(_) => None,
        }
    }

    /// Returns the `GeometryRef` if this batch uses shared geometry.
    pub fn shared_geometry(&self) -> Option<GeometryRef> {
        match &self.geometry {
            BatchGeometry::Shared(geo) => Some(*geo),
            BatchGeometry::Owned(_) => None,
        }
    }

    pub fn instances(&self) -> Option<&[u8]> {
        self
            .instance_batch
            .as_ref()
            .map(|instance_batch| instance_batch.instances.as_slice())
    }

    /// Returns the instance count (1 if no instance data).
    pub fn instance_count(&self) -> u32 {
        self.instance_batch
            .as_ref()
            .map(|instance_batch| instance_batch.len())
            .unwrap_or(1)
    }

    /// Returns the index count. For owned geometry, this is the number of
    /// indices in the batch. For shared geometry, the count is looked up
    /// from the `GeometryPool` at draw time — returns 0 here (the commander
    /// retrieves it from the pool).
    pub fn index_count(&self) -> u32 {
        match &self.geometry {
            BatchGeometry::Owned(vb) => vb.index_count(),
            BatchGeometry::Shared(_) => 0, // resolved from GeometryPool
        }
    }

    /// Whether this batch uses shared geometry.
    pub fn is_shared(&self) -> bool {
        matches!(self.geometry, BatchGeometry::Shared(_))
    }
}

#[derive(Debug)]
pub struct VertexBatch {
    pub(crate) vertices: Vec<u8>,
    pub(crate) vertex_stride: u32,
    pub(crate) indices: Vec<u16>,
}

impl VertexBatch {
    pub(crate) fn new(vertex_stride: u32) -> Self {
        Self {
            vertices: vec![],
            vertex_stride,
            indices: vec![]
        }
    }

    pub(crate) fn add_vertices<V: NoUninit>(&mut self, vertices: &[V], indices: &[u16]) {
        let offset = self.vertex_count();
        self.vertices.extend_from_slice(bytemuck::cast_slice(vertices));
        let offseted_indices =  
            indices.iter()
            .map(|index| index + offset as u16)
            .collect::<Vec<_>>()
        ;
        self.indices.extend_from_slice(&offseted_indices);
    }

    pub(crate) fn vertex_count(&self) -> u32 {
        self.vertices.len() as u32 / self.vertex_stride
    }

    pub(crate) fn index_count(&self) -> u32 {
        self.indices.len() as u32
    }
}


#[derive(Debug)]
pub struct InstanceBatch {
    pub(crate) instances: Vec<u8>,
    pub(crate) instance_stride: u32,
}

impl InstanceBatch {
    pub(crate) fn new(instance_stride: u32) -> Self {
        Self {
            instance_stride,
            instances: vec![]
        }
    }

    pub(crate) fn add_instances<I: NoUninit>(&mut self, instances: &[I]) {
        let type_size = std::mem::size_of::<I>();
        if type_size != self.instance_stride as usize {
            panic!("Instance size mismatch epxected: {}, found {}", self.instance_stride, type_size);
        }

        self.instances.extend_from_slice(bytemuck::cast_slice(instances));
    }

    pub(crate) fn len(&self) -> u32 {
        self.instances.len() as u32 / self.instance_stride
    }
}