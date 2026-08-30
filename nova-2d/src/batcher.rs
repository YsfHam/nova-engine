use std::collections::{BTreeMap, HashMap};

use nova_core::{assets::handle::Handle, graphics::{color::Color, draw_batch::DrawBatch, material::Material}, math::{Vec3Swizzles, vec3}};

use crate::{quad::Quad, vertex::Vertex2D};

const VERTICES_POSITIONS: [(f32, f32); 4] = [
    (-0.5, -0.5),
    (-0.5,  0.5),
    ( 0.5,  0.5),
    ( 0.5, -0.5),
];

pub struct Batcher2D {
    layers: BTreeMap<u32, BatchLayer>,
    quad_vertices: [Vertex2D; 4],
}

impl Batcher2D {
    pub fn new() -> Self {

        let quad_vertices = [
            Vertex2D {
                position: VERTICES_POSITIONS[0].into(),
                uv: [0.0, 0.0],
                color: Color::WHITE.into(),
            },

            Vertex2D {
                position: VERTICES_POSITIONS[1].into(),
                uv: [0.0, 1.0],
                color: Color::WHITE.into(),
            },

            Vertex2D {
                position: VERTICES_POSITIONS[2].into(),
                uv: [1.0, 1.0],
                color: Color::WHITE.into(),
            },

            Vertex2D {
                position: VERTICES_POSITIONS[3].into(),
                uv: [1.0, 0.0],
                color: Color::WHITE.into(),
            }
        ];

        Self {
            layers: BTreeMap::new(),
            quad_vertices,
        }
    }

    pub fn add_quad(&mut self, quad: Quad) {
        self.layers.entry(quad.z_index)
        .or_insert_with(|| BatchLayer::new())
        .add_quad(quad, &mut self.quad_vertices);
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

    fn add_quad(&mut self, quad: Quad, quad_vertices: &mut [Vertex2D]) {
        let transform = quad.transform();
        let uv = quad.uv;


        let uvs = [
            (uv.left, uv.top),    // TL
            (uv.left, uv.bottom), // BL
            (uv.right, uv.bottom), // BR
            (uv.right, uv.top)    // TR
        ];

        for (i, &(x, y)) in VERTICES_POSITIONS.iter().enumerate() {
            let vertex = &mut quad_vertices[i];
            vertex.position = transform.mul_vec3(vec3(x, y, 1.0)).xy().into();
            vertex.uv = uvs[i].into();
            vertex.color = quad.color.into();
        }


        let batch_index = *self.index_map.entry(quad.material)
        .or_insert_with(|| {
            self.batches.push(
                DrawBatch::new(
                    quad.material,
                    std::mem::size_of::<Vertex2D>() as u32
                )
            );
            self.batches.len() - 1
        });
        let batch = &mut self.batches[batch_index];
        batch.add_vertices(&quad_vertices, &[0, 1, 2, 0, 2, 3]);
    }
}