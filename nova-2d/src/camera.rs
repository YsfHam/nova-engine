use nova_core::math::{Mat4, Vec2};

use crate::utils::RectF32;

pub struct Camera2D {
    position: Vec2,
    viewport: RectF32,
}

impl Camera2D {
    pub fn new(position: Vec2, viewport: RectF32) -> Self {
        Self {
            position,
            viewport,
        }
    }

    pub fn with_position_and_size(position: Vec2, size: Vec2) -> Self {
        Self::new(position, RectF32 {
            top: 0.0,
            left: 0.0,
            bottom: size.y,
            right: size.x,
        })
    }

    pub fn projection(&self) -> Mat4 {
        let RectF32 {
            top,
            left,
            bottom,
            right,
        } = self.viewport;
        let view = Mat4::from_translation(-self.position.extend(0.0));
        let proj = Mat4::orthographic_lh(left, right, bottom, top, 0.0, 1.0);

        proj * view
    }
}