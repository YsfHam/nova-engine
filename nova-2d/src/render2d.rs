use nova_core::{assets::AssetsManager, graphics::{render_pass::RenderPassDescriptor, render_target::RenderTargetCommander}};

use crate::{batcher::Batcher2D, sprite::Sprite};

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

    pub fn draw(&mut self, sprite: Sprite) {
        self.batcher.add_sprite(sprite);
    }

    pub fn end_scene(self, pass_descriptor: RenderPassDescriptor, assets: &AssetsManager) {
        self.commander.submit_batches(pass_descriptor, self.batcher.into_iter(), assets);
    }
}