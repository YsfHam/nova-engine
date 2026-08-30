use nova_core::{assets::handle::Handle, graphics::{color::Color, material::Material}, math::{Mat3, Vec2, vec2}};

use crate::utils::RectF32;

pub struct Quad {
    pub position: Vec2,
    pub angle: f32,
    pub scale: Vec2,
    pub material: Handle<Material>,
    pub color: Color,
    pub z_index: u32,
    pub uv: RectF32,
}

impl Quad {

    pub fn new(material: Handle<Material>) -> Self {
        Self {
            material,
            color: Color::WHITE,
            z_index: 0,
            uv: RectF32 {
                top: 0.0,
                left: 0.0,
                bottom: 1.0,
                right: 1.0,
            },
            position: Vec2::ZERO,
            angle: 0.0,
            scale: vec2(1.0, 1.0),
        }
    }

    pub fn with_position(mut self, position: Vec2) -> Self {
        self.position = position;
        self
    }

    pub fn with_scale(mut self, scale: Vec2) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
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

    pub fn transform(&self) -> Mat3 {
        Mat3::from_scale_angle_translation(
            self.scale,
            self.angle,
            self.position
        )
    }
}