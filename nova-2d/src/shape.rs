use bytemuck::Pod;
use nova_core::{
    assets::handle::Handle,
    graphics::{color::Color, geometry::GeometryRef, material::Material},
    math::{Angle, Mat3, Vec2},
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

    /// The material type this shape is compatible with. The material's
    /// `MaterialTemplate::instance_layout` must match the byte layout of
    /// `InstanceData`.
    type Material: Material;

    /// The shared geometry all instances of this shape use.
    fn geometry() -> GeometryRef;

    /// Builds the instance data from common fields.
    fn build_instance(
        position: Vec2,
        rotation: Angle,
        scale: Vec2,
        color: Color,
        uv: RectF32,
    ) -> Self::InstanceData;
}

// ──────────────────────────────────────────────────────────────────────────
//  RectangleShape
// ──────────────────────────────────────────────────────────────────────────

/// A textured or solid rectangle. Uses the shared quad geometry with a
/// full 2D transform (position, rotation, scale) and UV rect.
///
/// Compatible material: [`SpriteMaterial`](crate::materials::SpriteMaterial).
pub struct RectangleShape;

impl Shape2D for RectangleShape {
    type InstanceData = crate::instance::RectInstance;
    type Material = crate::materials::SpriteMaterial;

    fn geometry() -> GeometryRef {
        crate::batcher::quad_geometry()
    }

    fn build_instance(
        position: Vec2,
        rotation: Angle,
        scale: Vec2,
        color: Color,
        uv: RectF32,
    ) -> Self::InstanceData {
        let transform = Mat3::from_scale_angle_translation(scale, rotation.into(), position);
        Self::InstanceData::new(transform, color, uv)
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  CircleShape
// ──────────────────────────────────────────────────────────────────────────

/// A filled circle. Inscribed in the shared quad geometry — the fragment
/// shader discards pixels outside the unit circle.
///
/// Compatible material: [`CircleMaterial`](crate::materials::CircleMaterial).
pub struct CircleShape;

impl Shape2D for CircleShape {
    type InstanceData = crate::instance::CircleInstance;
    type Material = crate::materials::CircleMaterial;

    fn geometry() -> GeometryRef {
        crate::batcher::quad_geometry()
    }

    fn build_instance(
        position: Vec2,
        _: Angle,
        scale: Vec2,
        color: Color,
        _: RectF32,
    ) -> Self::InstanceData {
        // The circle's radius is derived from the scale (average of x/y).
        // The quad spans -0.5..0.5, so scale maps directly to diameter.
        let radius = (scale.x + scale.y) * 0.25;
        Self::InstanceData::new(position, radius, color)
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
#[derive(Clone)]
pub struct ShapeInstance<S: Shape2D> {
    pub position: Vec2,
    pub angle: Angle,
    pub scale: Vec2,
    pub color: Color,
    pub uv: RectF32,
    pub material: Handle<S::Material>,
    pub z_index: u32,
}

impl<S: Shape2D> ShapeInstance<S> {
    pub fn new(material: Handle<S::Material>) -> Self {
        Self {
            position: Vec2::ZERO,
            angle: Angle::ZERO,
            scale: Vec2::new(1.0, 1.0),
            color: Color::WHITE,
            uv: RectF32 {
                top: 0.0,
                left: 0.0,
                bottom: 1.0,
                right: 1.0,
            },
            material,
            z_index: 0,
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

    pub fn with_angle(mut self, angle: Angle) -> Self {
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

    /// Builds the GPU instance data for this shape instance.
    pub fn build(&self) -> S::InstanceData {
        S::build_instance(self.position, self.angle, self.scale, self.color, self.uv)
    }
}