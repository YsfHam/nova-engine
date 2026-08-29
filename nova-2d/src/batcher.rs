use std::collections::{BTreeMap, HashMap};

use nova_core::{assets::handle::Handle, graphics::{draw_batch::{DrawBatch, VertexBatch}, material::Material}, math::vec4};

use crate::{quad::Quad, vertex::Vertex2D};

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
        .or_insert_with(|| BatchLayer::new())
        .add_quad(quad);
    }

    pub fn gen_batches(&self) -> impl Iterator<Item = DrawBatch> {
        self.layers.iter()
        .flat_map(|(_, layer)| {
            layer.quads.iter()
            .map(|(material, vbatch)| 
            DrawBatch::with_vertices(*material, vbatch.vertices(), vbatch.indices().to_vec()))
        })
    }
}

struct BatchLayer {
    quads: HashMap<Handle<Material>, VertexBatch>,
}

impl BatchLayer {
    fn new() -> Self {
        Self {
            quads: HashMap::new(),
        }
    }

    fn add_quad(&mut self, quad: Quad) {
        let transform = quad.transform();
        let uv = quad.uv;

        // Center-origin local coords: -0.5..0.5 on both axes.
        // The local origin (0,0) is the quad's center, so rotation in the
        // transform pivots around the center, not a corner.
        //
        // Vertex order [TL, BL, BR, TR] with indices [0,1,2, 0,2,3] produces
        // CCW triangles in NDC after the Y-flipping ortho projection,
        // matching the pipeline's `front_face: Ccw`.
        let pos_uv = [
            ((-0.5, -0.5), (uv.left, uv.top)),    // TL
            ((-0.5,  0.5), (uv.left, uv.bottom)), // BL
            (( 0.5,  0.5), (uv.right, uv.bottom)), // BR
            (( 0.5, -0.5), (uv.right, uv.top))    // TR
        ];

        let vertices = pos_uv.iter()
            .map(|((x, y), (uv_x, uv_y))| {
                Vertex2D {
                    position: transform.mul_vec4(vec4(*x, *y, 0.0, 1.0)).into(),
                    uv: [*uv_x, *uv_y],
                    color: quad.color.into(),
                }
            })
            .collect::<Vec<_>>();

        let batch = self.quads.entry(quad.material)
        .or_insert_with(|| VertexBatch::new(std::mem::size_of::<Vertex2D>() as u32));
        batch.add_vertices(&vertices, &[0, 1, 2, 0, 2, 3]);
    }
}