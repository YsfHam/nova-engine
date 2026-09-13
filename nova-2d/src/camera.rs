use nova_core::math::{Angle, Mat4, Quat, Vec2};

use crate::utils::RectF32;


pub struct Camera2D {
    pub position: Vec2,
    pub zoom: f32,
    pub rotation: Angle,

    viewport: RectF32,
}

impl Camera2D {
    pub fn with_viewport(viewport: RectF32) -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            rotation: Angle::ZERO,
            viewport,
        }
    }

    /// Creates a camera with a top-left origin: world `(0, 0)` maps to the
    /// top-left corner of the viewport. X increases rightward, Y increases
    /// downward.
    pub fn with_size(size: Vec2) -> Self {
        Self::with_viewport(RectF32 {
            top: 0.0,
            left: 0.0,
            bottom: size.y,
            right: size.x,
        })
    }

    /// Updates the viewport rect. Useful when the window is resized.
    pub fn set_viewport(&mut self, viewport: RectF32) {
        self.viewport = viewport;
    }

    /// Updates the viewport from a width/height pair (top-left origin).
    pub fn set_size(&mut self, size: Vec2) {
        self.viewport = RectF32 {
            top: 0.0,
            left: 0.0,
            bottom: size.y,
            right: size.x,
        };
    }

    pub fn projection(&self) -> Mat4 {
        let RectF32 {
            top,
            left,
            bottom,
            right,
        } = self.viewport;

        // Viewport center and half-extents.
        let center = Vec2::new((left + right) * 0.5, (top + bottom) * 0.5);
        let half_w = (right - left) * 0.5;
        let half_h = (bottom - top) * 0.5;

        
        let pivot = self.position + center;
        let view = Mat4::from_rotation_translation(
            Quat::from_rotation_z(f32::from(self.rotation)),
            pivot.extend(0.0),
        );

        // Centered orthographic projection: maps [-half, +half] to clip space.
        // Zoom shrinks the visible area so objects appear larger.
        let z = self.zoom;
        let proj = Mat4::orthographic_lh(
            -half_w / z,
            half_w / z,
            half_h / z,
            -half_h / z,
            0.0,
            1.0,
        );

        proj * view.inverse()
    }
}