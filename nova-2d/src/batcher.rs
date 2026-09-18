use std::{collections::{BTreeMap, HashMap}, sync::OnceLock};

use nova_core::{
    assets::handle::WeakGenericHandle,
    graphics::{draw_batch::DrawBatch, geometry::GeometryRef},
};

/// The shared base quad geometry (4 vertices + 6 indices). Set once at init
/// via [`set_quad_geometry`], then used by all batches.
static QUAD_GEOMETRY: OnceLock<GeometryRef> = OnceLock::new();

/// Sets the shared base quad geometry reference. Call once during plugin init.
pub fn set_quad_geometry(geo: GeometryRef) {
    let _ = QUAD_GEOMETRY.set(geo);
}

/// Returns the shared base quad geometry. Panics if not yet initialized.
pub fn quad_geometry() -> GeometryRef {
    QUAD_GEOMETRY
        .get()
        .copied()
        .expect("Batcher2D requires quad geometry — call set_quad_geometry first")
}

/// A type-erased instance batch: raw instance bytes + stride + the geometry
/// to use for the draw call. All instances in one `InstanceBatchRaw` share
/// the same material (and therefore the same shape type / instance layout).
struct InstanceBatchRaw {
    bytes: Vec<u8>,
    stride: u32,
    geometry: GeometryRef,
}

impl InstanceBatchRaw {
    fn new(stride: u32, geometry: GeometryRef) -> Self {
        Self {
            bytes: Vec::new(),
            stride,
            geometry,
        }
    }

    fn push<D: bytemuck::Pod>(&mut self, data: &D) {
        self.bytes
            .extend_from_slice(bytemuck::cast_slice(std::slice::from_ref(data)));
    }
}

/// One z-index layer. Holds per-material instance batches keyed by
/// `GenericHandle`. Each material maps to exactly one `InstanceBatchRaw`
/// — the material type determines the instance stride and geometry.
struct BatchLayer {
    index_map: HashMap<WeakGenericHandle, usize>,
    batches: Vec<(WeakGenericHandle, InstanceBatchRaw)>,
    capacity_hint: usize,
}

impl BatchLayer {
    fn new(capacity_hint: usize) -> Self {
        Self {
            index_map: HashMap::new(),
            batches: Vec::new(),
            capacity_hint,
        }
    }

    fn add<D: bytemuck::Pod>(
        &mut self,
        material: WeakGenericHandle,
        stride: u32,
        geometry: GeometryRef,
        data: &D,
    ) {
        let batch_index = *self.index_map.entry(material).or_insert_with(|| {
            let mut raw = InstanceBatchRaw::new(stride, geometry);
            if self.capacity_hint > 0 {
                raw.bytes.reserve(self.capacity_hint * stride as usize);
            }
            self.batches.push((material, raw));
            self.batches.len() - 1
        });
        let (_, batch) = &mut self.batches[batch_index];
        batch.push(data);
    }
}

pub struct Batcher2D {
    layers: BTreeMap<u32, BatchLayer>,
    capacity_hint: usize,
}

impl Batcher2D {
    pub fn new() -> Self {
        Self {
            layers: BTreeMap::new(),
            capacity_hint: 0,
        }
    }

    /// Pre-allocates capacity for `hint` instances per material group.
    pub fn reserve(&mut self, hint: usize) {
        self.capacity_hint = hint;
    }

    /// Adds a shape instance to the batch.
    ///
    /// The material handle encodes the material type, which determines the
    /// pipeline (shader + instance layout). The shape's `InstanceData` type
    /// must match the material's instance layout.
    pub fn add<S>(&mut self, material: WeakGenericHandle, data: &S::InstanceData, z_index: u32)
    where
        S: crate::shape::Shape2D,
    {
        let stride = std::mem::size_of::<S::InstanceData>() as u32;
        let geometry = S::geometry();

        self.layers
            .entry(z_index)
            .or_insert_with(|| BatchLayer::new(self.capacity_hint))
            .add(material, stride, geometry, data);
    }

    /// Consumes the batcher and produces `DrawBatch`es in z-index order,
    /// grouped by material within each layer.
    pub fn into_iter(self) -> impl Iterator<Item = DrawBatch> {
        self.layers
            .into_iter()
            .flat_map(|(_, layer)| layer.batches.into_iter())
            .map(|(material, raw)| {
                DrawBatch::with_shared_geometry(material, raw.geometry)
                    .with_instances_raw(&raw.bytes, raw.stride)
            })
    }
}