use nova_core::{
    assets::AssetsManager,
    graphics::{render_pass::RenderPassDescriptor, render_target::RenderTargetCommander},
};

use crate::{batcher::Batcher2D, shape::{Shape2D, ShapeInstance}};

pub struct Render2D<'a> {
    commander: RenderTargetCommander<'a>,
    batcher: Batcher2D,
}

impl<'a> Render2D<'a> {
    pub fn begin_scene(commander: RenderTargetCommander<'a>) -> Self {
        Self {
            commander,
            batcher: Batcher2D::new(),
        }
    }

    /// Pre-allocates capacity for `hint` instances per material group.
    pub fn reserve(&mut self, hint: usize) {
        self.batcher.reserve(hint);
    }

    /// Draws a shape instance. The shape type `S` determines the instance
    /// data layout and geometry; the material handle must be compatible
    /// with `S::Material`.
    pub fn draw<S: Shape2D>(&mut self, instance: &ShapeInstance<S>) {
        let data = instance.instance();
        self.batcher.add::<S>(instance.material, &data, instance.z_index);
    }

    pub fn end_scene(self, pass_descriptor: RenderPassDescriptor, assets: &AssetsManager) {
        self.commander
            .submit_batches(pass_descriptor, self.batcher.into_iter(), assets);
    }
}