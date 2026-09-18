
use std::ops::{Deref, DerefMut};

use bytemuck::Pod;
use nova_core::{
    assets::handle::{WeakGenericHandle, WeakHandle}, graphics::{color::Color, geometry::GeometryRef, material::Material}, math::{Angle, Vec2, vec2},
};

use crate::utils::RectF32;

/// A 2D shape that can be submitted to [`Render2D`](crate::render2d::Render2D).
///
/// A shape is **tied to a material type** — the associated `Material` type
/// declares the full `MaterialTemplate` (shader, instance layout, bind group
/// layout, blend state). The shape's `InstanceData` must match that
/// template's instance layout.
///
/// The shape provides:
/// - `InstanceData`: the per-instance GPU struct (`Pod`) — position, size,
///   color, UVs, or any other per-instance fields the shader needs.
/// - `Material`: the material type whose `MaterialTemplate` matches this
///   shape's instance layout and shader.
///
/// Built-in shapes:
/// - [`RectangleShape`] → [`SpriteMaterial`](crate::materials::SpriteMaterial)
/// - [`CircleShape`] → [`CircleMaterial`](crate::materials::CircleMaterial)
///
/// Users can add their own by implementing `Shape2D` + a compatible
/// `Material` + a WGSL shader.
pub trait Shape2D: Send + Sync + 'static {
    /// The per-instance GPU data struct. Must be `Pod` (plain-old-data) so it
    /// can be uploaded directly to the instance buffer.
    type InstanceData: Pod + Clone + Send + Sync + 'static;

    /// The shared geometry all instances of this shape use.
    fn geometry() -> GeometryRef;

    /// Builds the instance data from common fields.
    fn instance(&self) -> Self::InstanceData;
}

// ──────────────────────────────────────────────────────────────────────────
//  RectangleShape
// ──────────────────────────────────────────────────────────────────────────

/// A textured or solid rectangle. Uses the shared quad geometry with a
/// full 2D transform (position, rotation, scale) and UV rect.
///
/// Compatible material: [`SpriteMaterial`](crate::materials::SpriteMaterial).
pub struct RectangleShape {
    pub position: Vec2,
    pub angle: Angle,
    pub scale: Vec2,
    pub color: Color,
    pub uv: RectF32,
}

impl Default for RectangleShape {
    fn default() -> Self {
        Self {
            position: Vec2::default(),
            angle: Angle::Radians(0.0),
            scale: vec2(1.0, 1.0),
            color: Color::WHITE,
            uv: RectF32 {
                top: 0.0,
                left: 0.0,
                bottom: 1.0,
                right: 1.0,
            },
        }
    }
}

impl RectangleShape {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_position(mut self, position: Vec2) -> Self {
        self.position = position;
        self
    }

    pub fn with_angle(mut self, angle: Angle) -> Self {
        self.angle = angle;
        self
    }

    pub fn with_scale(mut self, scale: Vec2) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_uv(mut self, uv: RectF32) -> Self {
        self.uv = uv;
        self
    }
}


impl Shape2D for RectangleShape {
    type InstanceData = crate::instance::RectInstance;

    fn geometry() -> GeometryRef {
        crate::batcher::quad_geometry()
    }

    fn instance(&self) -> Self::InstanceData {
        Self::InstanceData {
            position: self.position.into(),
            scale: self.scale.into(),
            color: self.color.into(),
            uv_rect: [
                self.uv.left,
                self.uv.top,
                self.uv.right,
                self.uv.bottom
            ],
            rotation: self.angle.into(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  CircleShape
// ──────────────────────────────────────────────────────────────────────────

/// A filled circle. Inscribed in the shared quad geometry — the fragment
/// shader discards pixels outside the unit circle.
///
/// Compatible material: [`CircleMaterial`](crate::materials::CircleMaterial).
pub struct CircleShape {
    pub position: Vec2,
    pub radius: f32,
    pub color: Color,
}

impl Default for CircleShape {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            radius: 1.0,
            color: Color::WHITE
        }
    }
}

impl CircleShape {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_position(mut self, position: Vec2) -> Self {
        self.position = position;
        self
    }

    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl Shape2D for CircleShape {
    type InstanceData = crate::instance::CircleInstance;

    fn geometry() -> GeometryRef {
        crate::batcher::quad_geometry()
    }

    fn instance(&self) -> Self::InstanceData {
        Self::InstanceData {
            position: self.position.into(),
            radius: self.radius,
            color: self.color.into(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  ShapeInstance — a shape + transform + material ready to draw
// ──────────────────────────────────────────────────────────────────────────

/// A shape instance ready to be submitted to the renderer.
///
/// Bundles the shape's common fields (position, angle, scale, color, UV)
/// with a material handle and z-index. The renderer calls
/// `Shape2D::build_instance` to produce the GPU instance data.
pub struct ShapeInstance<S: Shape2D> {
    pub shape: S,
    pub material: WeakGenericHandle,
    pub z_index: u32,
}

impl<S: Shape2D> ShapeInstance<S> {
    pub fn new<M: Material>(shape: S, material: WeakHandle<M>) -> Self {
        Self {
            shape,
            material: material.into_generic(),
            z_index: 0,
        }
    }

    pub fn with_z_index(mut self, z_index: u32) -> Self {
        self.z_index = z_index;
        self
    }

    /// Builds the GPU instance data for this shape instance.
    pub fn instance(&self) -> S::InstanceData {
        self.shape.instance()
    }
}

impl<S: Shape2D> Deref for ShapeInstance<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.shape
    }
}

impl<S: Shape2D> DerefMut for ShapeInstance<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.shape
    }
}