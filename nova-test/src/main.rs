use nova_core::app::{ApplicationBuilder, ApplicationContext, ApplicationProxy};

pub struct AppProxy;

impl ApplicationProxy for AppProxy {
    fn on_update(&mut self, _ctx: &ApplicationContext) {
    }
}

fn main() {
    ApplicationBuilder::new(AppProxy)
    .build()
    .run();
}
