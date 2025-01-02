use std::num::NonZeroU32;
use std::process::{Child, Command};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use helper::default_typeface;
use html::HtmlElement;
use skia_safe::{Font, Paint, Rect, TextBlob};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

mod helper;
mod html;

#[derive(Default)]
struct App {
    html: Arc<Mutex<Option<HtmlElement>>>,
    window: Arc<Mutex<Option<Window>>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Arc::new(Mutex::new(Some(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        )));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                {
                    let html_guard = self.html.lock().unwrap();

                    let title_text = Mutex::new(String::new());
                    let body_text = Mutex::new(String::new());
                    if let Some(html) = html_guard.as_ref() {
                        html.walk(Rc::new(|name, _, children: Vec<HtmlElement>, _| {
                            if name == "title" {
                                for child in children {
                                    if let Some(text_node) = child.text_node {
                                        title_text.lock().unwrap().push_str(&text_node);
                                    }
                                }
                            } else if name == "body" {
                                for child in children {
                                    if let Some(text_node) = child.text_node {
                                        body_text.lock().unwrap().push_str(&text_node);
                                    }
                                }
                            }
                        }));
                    }
                    let body_text = body_text.lock().unwrap();

                    let window_lock = self.window.lock();
                    let window_guard = window_lock.as_ref().unwrap();
                    let window = window_guard.as_ref().unwrap();
                    let context = softbuffer::Context::new(&window).unwrap();
                    let mut surface = softbuffer::Surface::new(&context, &window).unwrap();

                    let (width, height) = {
                        let size = window.inner_size();
                        (size.width, size.height)
                    };
                    surface
                        .resize(
                            NonZeroU32::new(width).unwrap(),
                            NonZeroU32::new(height).unwrap(),
                        )
                        .unwrap();

                    let mut raster_surface =
                        skia_safe::surfaces::raster_n32_premul((width as i32, height as i32))
                            .unwrap();
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
                        "http://localhost:8000",
                        &Font::from_typeface(default_typeface(), 32.0),
                    )
                    .unwrap();

                    paint.set_argb(0xFF, 0x00, 0x00, 0x00);
                    canvas.draw_text_blob(&text, (25, 60 + 36), &paint);

                    let text = TextBlob::from_str(
                        title_text.lock().unwrap().as_str(),
                        &Font::from_typeface(default_typeface(), 32.0),
                    );
                    if let Some(text) = text {
                        paint.set_argb(0xFF, 0x00, 0x00, 0x00);
                        canvas.draw_text_blob(&text, (25, 5 + 32), &paint);
                    }

                    let text = TextBlob::from_str(
                        if body_text.len() > 0 {
                            &body_text
                        } else {
                            "Loading..."
                        },
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
                }

                let html = self.html.clone();
                let window = self.window.clone();

                if self.html.clone().lock().unwrap().is_none() {
                    tokio::spawn(async move {
                        let resp = fetch().await.unwrap();
                        *html.lock().unwrap() = Some(html::parse_html(resp).unwrap());

                        let window = window.lock().unwrap();
                        window.as_ref().unwrap().request_redraw();
                    });
                }
            }
            _ => (),
        }
    }
}

async fn fetch() -> Result<String, Box<dyn std::error::Error>> {
    let resp = reqwest::get("http://localhost:8000").await?.text().await?;

    Ok(resp)
}

struct DroppableProcess {
    child: Child,
}

impl DroppableProcess {
    fn new(command: &mut Command) -> Result<Self, Box<dyn std::error::Error>> {
        let child = command.spawn()?;
        Ok(Self { child })
    }
}

impl Drop for DroppableProcess {
    fn drop(&mut self) {
        println!("Killing child process");

        if let Err(err) = self.child.kill() {
            eprintln!("Failed to kill child process: {}", err);
        }
    }
}

async fn ensure_server_started(url: &str, timeout: std::time::Duration) -> Result<(), String> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(response) = reqwest::get(url).await {
            if response.status().is_success() {
                return Ok(());
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    Err("Server did not start within the timeout".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // extend the lifetime of the process to the end of the program
    let _process: DroppableProcess = DroppableProcess::new(
        Command::new("python")
            .arg("-m")
            .arg("http.server")
            .arg("8000")
            .arg("-d")
            .arg("public"),
    )?;
    ensure_server_started("http://localhost:8000", std::time::Duration::from_secs(5)).await?;

    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();

    Ok(())
}
