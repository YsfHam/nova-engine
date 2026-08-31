use std::collections::{BTreeMap, HashMap};

use nova_core::{assets::handle::Handle, graphics::{draw_batch::DrawBatch, geometry::GeometryRef, material::Material}};

use crate::{instance::InstanceData2D, quad::Quad};

/// The shared base quad geometry (4 vertices + 6 indices). Set once at init
/// via [`Batcher2D::set_quad_geometry`], then used by all batches.
/// When `None`, `add_quad` panics — the batcher must be initialized first.
static mut QUAD_GEOMETRY: Option<GeometryRef> = None;

/// Sets the shared base quad geometry reference. Call once during plugin init
/// (after the `Nova2DPlugin` registers the geometry). This is a global static
/// because `Batcher2D::new()` needs it without threading a reference through
/// every `Render2D::begin_scene` call.
///
/// TODO: This should be on `Render2D` or passed via the commander instead of
/// a global. For now, the static keeps the API simple.
pub fn set_quad_geometry(geo: GeometryRef) {
    unsafe {
        QUAD_GEOMETRY = Some(geo);
    }
}

fn quad_geometry() -> GeometryRef {
    unsafe {
        QUAD_GEOMETRY.expect("Batcher2D requires quad geometry — call set_quad_geometry first")
    }
}

pub struct Batcher2D {
    layers: BTreeMap<u32, BatchLayer>,
    /// Hint: expected number of quads per material group. Used to pre-allocate
    /// instance vectors, avoiding repeated reallocations as they grow.
    /// Set via [`Batcher2D::reserve`]. Defaults to 0 (grow on demand).
    capacity_hint: usize,
}

impl Batcher2D {
    pub fn new() -> Self {
        Self {
            layers: BTreeMap::new(),
            capacity_hint: 0,
        }
    }

    /// Pre-allocates capacity for `hint` instances per material group. Call
    /// before `add_quad` if you know (or can estimate) the quad count —
    /// eliminates the reallocation churn as `Vec<InstanceData2D>` grows.
    pub fn reserve(&mut self, hint: usize) {
        self.capacity_hint = hint;
    }

    pub fn add_quad(&mut self, quad: Quad) {
        self.layers.entry(quad.z_index)
            .or_insert_with(BatchLayer::new)
            .add_quad(quad, self.capacity_hint);
    }

    pub fn into_iter(self) -> impl Iterator<Item = DrawBatch> {
        self.layers
            .into_iter()
            .flat_map(|(_, layer)| {
                layer.instances
            })
            .map(|(material, instances)| {
                DrawBatch::with_shared_geometry(material, quad_geometry())
                .with_instances(&instances, std::mem::size_of::<InstanceData2D>() as u32)
            })
    }
}

struct BatchLayer {
    index_map: HashMap<Handle<Material>, usize>,
    instances: Vec<(Handle<Material>, Vec<InstanceData2D>)>,
}

impl BatchLayer {
    fn new() -> Self {
        Self {
            index_map: HashMap::new(),
            instances: Vec::new(),
        }
    }

    fn add_quad(&mut self, quad: Quad, capacity_hint: usize) {
        // Build the per-instance data: transform, color, UV rect.
        let instance = InstanceData2D::new(quad.transform(), quad.color, quad.uv);

        // Find or create the batch for this material.
        let batch_index = *self.index_map.entry(quad.material)
            .or_insert_with(|| {
                let mut instances = Vec::new();
                if capacity_hint > 0 {
                    instances.reserve(capacity_hint);
                }
                self.instances.push(
                    (quad.material, instances)
                );
                self.instances.len() - 1
            });
        let (_, instances) = &mut self.instances[batch_index];

        // Add the instance data.
        instances.push(instance);
    }
}