use std::num::NonZeroU32;
use std::process::{Child, Command};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use helper::default_typeface;
use html::{HtmlElement, NodeTrace};
use skia_safe::{Font, Paint, Rect, TextBlob};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

mod helper;
mod html;

#[derive(Default)]
struct App {
    path: String,
    html: Arc<Mutex<Option<HtmlElement>>>,
    window: Arc<Mutex<Option<Window>>>,
    mouse_cursor_position: Mutex<(f32, f32)>,
    hyper_links: Arc<Mutex<Vec<(Rect, String)>>>,
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
                    canvas.clear(0xFFFFFFFF);

                    let mut paint = Paint::default();

                    paint.set_argb(0xFF, 0x99, 0x99, 0x99);
                    canvas.draw_rect(Rect::new(0.0, 0.0, width as f32, 50.0), &paint);

                    paint.set_argb(0xFF, 0xDD, 0xDD, 0xDD);
                    canvas.draw_rect(Rect::new(0.0, 50.0, width as f32, 120.0), &paint);

                    paint.set_argb(0xFF, 0xFF, 0xFF, 0xFF);
                    canvas.draw_rect(Rect::new(20.0, 60.0, 1000.0, 110.0), &paint);

                    let text = TextBlob::from_str(
                        format!("http://localhost:8000/{}", self.path),
                        &Font::from_typeface(default_typeface(), 32.0),
                    )
                    .unwrap();

                    paint.set_argb(0xFF, 0x00, 0x00, 0x00);
                    canvas.draw_text_blob(&text, (25, 60 + 36), &paint);

                    if let Some(html) = self.html.lock().unwrap().as_ref() {
                        let cursor_position = Mutex::new((25.0, 120.0 + 36.0));
                        let hyper_links = self.hyper_links.clone();
                        html.walk(Rc::new(
                            move |trace: NodeTrace,
                                  name,
                                  attributes: Vec<(String, String)>,
                                  children: Vec<HtmlElement>,
                                  text_node: Option<String>| {
                                println!("{:?} ({:?})", trace, name);
                                let mut paint = Paint::default();

                                if trace.names().ends_with(&["title".to_string()]) {
                                    let mut title = String::new();
                                    for child in children {
                                        title.push_str(&child.text_node.unwrap());
                                        title.push_str(" ");
                                    }

                                    println!("Title: {}", title);

                                    let text = TextBlob::from_str(
                                        title,
                                        &Font::from_typeface(default_typeface(), 32.0),
                                    );
                                    if let Some(text) = text {
                                        paint.set_argb(0xFF, 0x00, 0x00, 0x00);
                                        canvas.draw_text_blob(&text, (25, 5 + 32), &paint);
                                    }
                                } else if trace.names().contains(&"body".to_string()) {
                                    let is_anchor = trace.names().ends_with(&["a".to_string()]);
                                    if let Some(text_node) = text_node {
                                        let mut paint = Paint::default();
                                        let font = Font::from_typeface(default_typeface(), 32.0);

                                        let text = TextBlob::from_str(&text_node, &font);
                                        if let Some(text) = text {
                                            if is_anchor {
                                                paint.set_argb(0xFF, 0x00, 0x55, 0xFF);
                                            } else {
                                                paint.set_argb(0xFF, 0x00, 0x00, 0x00);
                                            }
                                            let pos = *cursor_position.lock().unwrap();
                                            canvas.draw_text_blob(&text, (pos.0, pos.1), &paint);

                                            if is_anchor {
                                                let (_, rect) =
                                                    font.measure_str(&text_node, Some(&paint));

                                                println!("Hyperlink: {:?}", attributes);

                                                hyper_links.lock().unwrap().push((
                                                    Rect::new(
                                                        pos.0,
                                                        pos.1 - 32.0,
                                                        pos.0 + rect.width(),
                                                        pos.1 + rect.height() - 32.0,
                                                    ),
                                                    trace
                                                        .0
                                                        .last()
                                                        .unwrap()
                                                        .1
                                                        .iter()
                                                        .find(|(key, _)| key == "href")
                                                        .unwrap()
                                                        .1
                                                        .clone(),
                                                ));
                                            }

                                            let (_, rect) =
                                                font.measure_str(text_node + " ", Some(&paint));

                                            *cursor_position.lock().unwrap() =
                                                (pos.0 + rect.width() + 8.0, pos.1);
                                        }
                                    }

                                    if name == "br" {
                                        let pos = *cursor_position.lock().unwrap();
                                        *cursor_position.lock().unwrap() = (25.0, pos.1 + 36.0);
                                    }
                                }
                            },
                        ));
                    }

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
                    let path = self.path.clone();
                    tokio::spawn(async move {
                        let resp = fetch(path).await.unwrap();
                        *html.lock().unwrap() = Some(html::parse_html(resp).unwrap());

                        let window = window.lock().unwrap();
                        window.as_ref().unwrap().request_redraw();
                    });
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                *self.mouse_cursor_position.lock().unwrap() =
                    (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput { .. } => {
                let pos = self.mouse_cursor_position.lock().unwrap();
                let links = self.hyper_links.lock().unwrap().clone();

                for (link, path) in links.iter() {
                    if link.x() <= pos.0
                        && pos.0 <= link.right()
                        && link.y() <= pos.1
                        && pos.1 <= link.bottom()
                    {
                        self.path = path.clone();
                        self.html = Arc::new(Mutex::new(None));
                        self.hyper_links.lock().unwrap().clear();

                        let window = self.window.lock().unwrap();
                        window.as_ref().unwrap().request_redraw();
                    }
                }
            }
            _ => (),
        }
    }
}

async fn fetch(path: String) -> Result<String, Box<dyn std::error::Error>> {
    let resp = reqwest::get(format!("http://localhost:8000/{}", path))
        .await?
        .text()
        .await?;

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
