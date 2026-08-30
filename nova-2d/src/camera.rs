use nova_core::math::{Mat4, Vec2};


pub struct Camera2D {
    position: Vec2,
    size: Vec2,
}

impl Camera2D {
    pub fn with_position_and_size(position: Vec2, size: Vec2) -> Self {
        Self {
            position,
            size,
        }
    }

    pub fn with_size(size: Vec2) -> Self {
        Self::with_position_and_size(Vec2::ZERO, size)
    }

    pub fn projection(&self) -> Mat4 {
        let view = Mat4::from_translation(-self.position.extend(0.0));
        let proj = Mat4::orthographic_lh(0.0, self.size.x, self.size.y, 0.0, 0.0, 1.0);

        proj * view
    }
}