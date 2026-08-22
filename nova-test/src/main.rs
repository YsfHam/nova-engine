use nova_core::{EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}};

pub struct AppProxy;

impl ApplicationProxy for AppProxy {
    fn on_update(&mut self, ctx: &ApplicationContext, dt: std::time::Duration) {
    }
}

fn main() -> EngineResult<()> {
    simple_logger::init().unwrap();

    ApplicationBuilder::new(AppProxy)
    .build()
    .run()
}
