use std::num::NonZeroU32;

use helper::default_typeface;
use skia_safe::{Font, Paint, Rect, TextBlob};
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
                let mut paint = Paint::default();

                canvas.clear(0xFFFFFFFF);

                paint.set_argb(0xFF, 0x99, 0x99, 0x99);
                canvas.draw_rect(Rect::new(0.0, 0.0, width as f32, 50.0), &paint);

                paint.set_argb(0xFF, 0xDD, 0xDD, 0xDD);
                canvas.draw_rect(Rect::new(0.0, 50.0, width as f32, 120.0), &paint);

                paint.set_argb(0xFF, 0xFF, 0xFF, 0xFF);
                canvas.draw_rect(Rect::new(20.0, 60.0, 1000.0, 110.0), &paint);

                let text = TextBlob::from_str(
                    "http://example.com",
                    &Font::from_typeface(default_typeface(), 36.0),
                )
                .unwrap();

                paint.set_argb(0xFF, 0x00, 0x00, 0x00);
                canvas.draw_text_blob(&text, (25, 60 + 36), &paint);

                let text = TextBlob::from_str(
                    "Hello, from GUI!",
                    &Font::from_typeface(default_typeface(), 36.0),
                )
                .unwrap();

                paint.set_argb(0xFF, 0x00, 0x00, 0x00);
                canvas.draw_text_blob(&text, (20, 140 + 36), &paint);

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

async fn fetch() -> Result<(), Box<dyn std::error::Error>> {
    let resp = reqwest::get("http://localhost:8000").await?.text().await?;
    println!("Response: {resp}");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    fetch().await?;

    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();

    Ok(())
}
