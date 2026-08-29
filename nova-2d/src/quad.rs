use nova_core::{assets::handle::Handle, graphics::{color::Color, material::Material}, math::{Mat4, Vec2}};

use crate::utils::RectF32;

pub struct Quad {
    pub transform: Mat4,
    pub material: Handle<Material>,
    pub color: Color,
    pub z_index: u32,
    pub uv: RectF32,
}

impl Quad {

    pub fn new(material: Handle<Material>) -> Self {
        Self {
            transform: Default::default(),
            material,
            color: Color::WHITE,
            z_index: 1,
            uv: RectF32 {
                top: 0.0,
                left: 0.0,
                bottom: 1.0,
                right: 1.0,
            },
        }
    }

    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    pub fn translate(mut self, position: Vec2) -> Self {
        self.transform = self.transform.mul_mat4(&Mat4::from_translation(position.extend(0.0)));
        self
    }

    pub fn scale(mut self, scale: Vec2) -> Self {
        self.transform = self.transform.mul_mat4(&Mat4::from_scale(scale.extend(1.0)));
        self
    }

    pub fn rotate(mut self, angle: f32) -> Self {
        self.transform = self.transform.mul_mat4(&Mat4::from_rotation_z(angle));
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_z_index(mut self, z_index: u32) -> Self {
        self.z_index = z_index;
        self
    }

    pub fn with_uv(mut self, uv: RectF32) -> Self {
        self.uv = uv;
        self
    }
}