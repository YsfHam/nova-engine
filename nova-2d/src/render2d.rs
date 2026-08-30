use nova_core::{assets::AssetsManager, graphics::{render_pass::RenderPassDescriptor, render_target::RenderTargetCommander}};

use crate::{batcher::Batcher2D, quad::Quad};

pub struct Render2D<'a> {
    commander: RenderTargetCommander<'a>,
    batcher: Batcher2D,
}

impl<'a> Render2D<'a> {
    pub fn begin_scene(
        commander: RenderTargetCommander<'a>,
    ) -> Self {
        Self {
            commander,
            batcher: Batcher2D::new(),
        }
    }

    pub fn draw_quad(&mut self, quad: Quad) {
        self.batcher.add_quad(quad);
    }

    pub fn end_scene(self, pass_descriptor: RenderPassDescriptor, assets: &AssetsManager) {
        self.commander.submit_batches(pass_descriptor, self.batcher.into_iter(), assets);
    }
}