use std::collections::{BTreeMap, HashMap};

use nova_core::{assets::handle::GenericHandle, graphics::{draw_batch::DrawBatch, geometry::GeometryRef}};

use crate::{instance::InstanceData2D, sprite::Sprite};

/// The shared base quad geometry (4 vertices + 6 indices). Set once at init
/// via [`set_quad_geometry`], then used by all batches.
static mut QUAD_GEOMETRY: Option<GeometryRef> = None;

/// Sets the shared base quad geometry reference. Call once during plugin init.
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

    pub fn add_sprite(&mut self, sprite: Sprite) {
        self.layers.entry(sprite.z_index)
            .or_insert_with(BatchLayer::new)
            .add_sprite(sprite, self.capacity_hint);
    }

    pub fn into_iter(self) -> impl Iterator<Item = DrawBatch> {
        self.layers
            .into_iter()
            .flat_map(|(_, layer)| layer.instances)
            .map(|(material, instances)| {
                DrawBatch::with_shared_geometry(material, quad_geometry())
                    .with_instances(&instances, std::mem::size_of::<InstanceData2D>() as u32)
            })
    }
}

struct BatchLayer {
    index_map: HashMap<GenericHandle, usize>,
    instances: Vec<(GenericHandle, Vec<InstanceData2D>)>,
}

impl BatchLayer {
    fn new() -> Self {
        Self {
            index_map: HashMap::new(),
            instances: Vec::new(),
        }
    }

    fn add_sprite(&mut self, sprite: Sprite, capacity_hint: usize) {
        let instance = InstanceData2D::new(sprite.transform(), sprite.color, sprite.uv);

        let batch_index = *self.index_map.entry(sprite.material)
            .or_insert_with(|| {
                let mut instances = Vec::new();
                if capacity_hint > 0 {
                    instances.reserve(capacity_hint);
                }
                self.instances.push((sprite.material, instances));
                self.instances.len() - 1
            });
        let (_, instances) = &mut self.instances[batch_index];
        instances.push(instance);
    }
}