use std::{sync::Arc, time::Instant};

use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::ControlFlow};

use crate::{app::{Application, ApplicationProxy}, graphics::context::GraphicsContext};

impl<P: ApplicationProxy> ApplicationHandler for Application<P> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
       if let Err(error) = self.init(event_loop) {
            self.engine_error = Some(error);
            event_loop.exit();
       }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {

        let ctx = self.ctx.as_mut().unwrap();
        let proxy = &mut self.proxy;

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::RedrawRequested => {
                let mut dt = self.frame_clock.restart();
                while dt >= self.frame_time {
                    proxy.on_update(ctx, self.frame_time);
                    dt -= self.frame_time;
                }
                render(&mut ctx.gfx);

                event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + self.frame_time));
            }

            WindowEvent::Resized(size) => {
                ctx.gfx.resize_surface(size.width, size.height);
            }

            _ => ()
        }
    }

    fn new_events(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, _cause: winit::event::StartCause) {
        if self.frame_clock.elapsed() >= self.frame_time {
            self.ctx.as_ref().map(|ctx| ctx.request_window_redraw());
        }
    }
}

fn render(gfx: &mut Arc<GraphicsContext>) {
    let output = match gfx.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                surface_texture
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                // Skip this frame
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                gfx.configure_surface();
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                *gfx = pollster::block_on(gfx.reconfigure()).unwrap().into();
                return;
            }
        };

    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = gfx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });


        {
        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
    }

    // submit will accept anything that implements IntoIter
    gfx.queue.submit(std::iter::once(encoder.finish()));
    gfx.queue.present(output);

}