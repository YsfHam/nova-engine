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
}

impl Batcher2D {
    pub fn new() -> Self {
        Self {
            layers: BTreeMap::new(),
        }
    }

    pub fn add_quad(&mut self, quad: Quad) {
        self.layers.entry(quad.z_index)
            .or_insert_with(BatchLayer::new)
            .add_quad(quad);
    }

    pub fn into_iter(self) -> impl Iterator<Item = DrawBatch> {
        self.layers
            .into_iter()
            .flat_map(|(_, layer)| layer.batches)
    }
}

struct BatchLayer {
    index_map: HashMap<Handle<Material>, usize>,
    batches: Vec<DrawBatch>,
}

impl BatchLayer {
    fn new() -> Self {
        Self {
            index_map: HashMap::new(),
            batches: Vec::new(),
        }
    }

    fn add_quad(&mut self, quad: Quad) {
        // Build the per-instance data: transform, color, UV rect.
        let instance = InstanceData2D::new(quad.transform(), quad.color, quad.uv);

        // Find or create the batch for this material.
        let batch_index = *self.index_map.entry(quad.material)
            .or_insert_with(|| {
                let geo = quad_geometry();
                self.batches.push(
                    DrawBatch::with_shared_geometry(quad.material, geo)
                );
                self.batches.len() - 1
            });
        let batch = &mut self.batches[batch_index];

        // Add the instance data. The batch references shared geometry for
        // vertices/indices (uploaded once) and accumulates per-instance data
        // (uploaded each frame).
        batch.add_instances(&[instance]);
    }
}