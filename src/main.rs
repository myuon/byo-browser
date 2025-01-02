use std::num::NonZeroU32;

use helper::default_typeface;
use skia_safe::{Font, Paint, TextBlob};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

mod helper;

#[derive(Default)]
struct App {
    window: Option<Window>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let context = softbuffer::Context::new(self.window.as_ref().unwrap()).unwrap();
                let mut surface =
                    softbuffer::Surface::new(&context, self.window.as_ref().unwrap()).unwrap();

                let (width, height) = {
                    let size = self.window.as_ref().unwrap().inner_size();
                    (size.width, size.height)
                };
                surface
                    .resize(
                        NonZeroU32::new(width).unwrap(),
                        NonZeroU32::new(height).unwrap(),
                    )
                    .unwrap();

                let mut raster_surface =
                    skia_safe::surfaces::raster_n32_premul((width as i32, height as i32)).unwrap();
                let canvas = raster_surface.canvas();
                let paint = Paint::default();

                canvas.clear(0xFFFFFFFF);

                let text = TextBlob::from_str(
                    "Hello, Skia!",
                    &Font::from_typeface(default_typeface(), 36.0),
                )
                .unwrap();

                canvas.draw_text_blob(&text, (50, 45), &paint);

                let pixdata = canvas.peek_pixels().unwrap();
                let pixdata = pixdata.bytes().unwrap();

                let mut buffer = surface.buffer_mut().unwrap();
                for index in 0..(width * height) as usize {
                    buffer[index] = pixdata[index * 4 + 2] as u32
                        | (pixdata[index * 4 + 1] as u32) << 8
                        | (pixdata[index * 4 + 0] as u32) << 16;
                }
                buffer.present().unwrap();

                // self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
