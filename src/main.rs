use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use helper::default_typeface;
use skia_safe::{Font, Paint, Rect, TextBlob};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

mod helper;

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
                        *html.lock().unwrap() = Some(parse_html(resp));

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

#[derive(Debug, PartialEq, Eq, Clone)]
enum Token {
    LAngle,
    RAngle,
    Slash,
    Equal,
    Text(String),
    QuotedText(String),
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct HtmlElement {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<HtmlElement>,
    text_node: Option<String>,
}

impl HtmlElement {
    pub fn walk<F: Fn(String, Vec<(String, String)>, Vec<HtmlElement>, Option<String>)>(
        &self,
        f: Rc<F>,
    ) {
        f(
            self.name.clone(),
            self.attributes.clone(),
            self.children.clone(),
            self.text_node.clone(),
        );

        for child in &self.children {
            child.walk(f.clone());
        }
    }
}

fn tokenize_html(str: String) -> Vec<Token> {
    let mut tokens = vec![];
    let chars = str.chars().collect::<Vec<_>>();
    let mut position = 0;

    while position < chars.len() {
        if chars[position].is_whitespace() {
            position += 1;
            continue;
        } else if chars[position] == '<' {
            tokens.push(Token::LAngle);
            position += 1;
        } else if chars[position] == '>' {
            tokens.push(Token::RAngle);
            position += 1;
        } else if chars[position] == '/' {
            tokens.push(Token::Slash);
            position += 1;
        } else if chars[position] == '=' {
            tokens.push(Token::Equal);
            position += 1;
        } else if chars[position] == '"' {
            let mut text = String::new();
            position += 1;
            while position < chars.len() {
                text.push(chars[position]);
                position += 1;

                if position < chars.len() && chars[position] == '"' {
                    break;
                }
            }
            tokens.push(Token::QuotedText(text));
            position += 1;
        } else {
            let mut text = String::new();
            text.push(chars[position]);
            position += 1;
            while position < chars.len()
                && chars[position] != ' '
                && chars[position] != '<'
                && chars[position] != '>'
                && chars[position] != '='
            {
                text.push(chars[position]);
                position += 1;
            }
            tokens.push(Token::Text(text));
        }
    }

    tokens
}

#[test]
fn test_tokenize_html() {
    let cases = vec![
        (
            "<html></html>",
            vec![
                Token::LAngle,
                Token::Text("html".to_string()),
                Token::RAngle,
                Token::LAngle,
                Token::Slash,
                Token::Text("html".to_string()),
                Token::RAngle,
            ],
        ),
        (
            r##"<body bgcolor="#ffffff">This is a paragraph</body>"##,
            vec![
                Token::LAngle,
                Token::Text("body".to_string()),
                Token::Text("bgcolor".to_string()),
                Token::Equal,
                Token::QuotedText("#ffffff".to_string()),
                Token::RAngle,
                Token::Text("This".to_string()),
                Token::Text("is".to_string()),
                Token::Text("a".to_string()),
                Token::Text("paragraph".to_string()),
                Token::LAngle,
                Token::Slash,
                Token::Text("body".to_string()),
                Token::RAngle,
            ],
        ),
    ];

    for (str, want) in cases {
        assert_eq!(tokenize_html(str.to_string()), want);
    }
}

struct HtmlParser {
    tokens: Vec<Token>,
    position: usize,
}

impl HtmlParser {
    fn new(tokens: Vec<Token>) -> Self {
        HtmlParser {
            tokens,
            position: 0,
        }
    }

    fn expect(&mut self, token: Token) {
        if self.tokens[self.position] != token {
            panic!(
                "Unexpected token: {:?} ({})",
                self.tokens[self.position], self.position
            );
        }
        self.position += 1;
    }

    fn expect_text(&mut self) -> String {
        if let Token::Text(text) = &self.tokens[self.position] {
            self.position += 1;
            text.clone()
        } else {
            panic!(
                "Unexpected token: {:?} ({})",
                self.tokens[self.position], self.position
            );
        }
    }

    fn expect_quoted_text(&mut self) -> String {
        if let Token::QuotedText(text) = &self.tokens[self.position] {
            self.position += 1;
            text.clone()
        } else {
            panic!(
                "Unexpected token: {:?} ({})",
                self.tokens[self.position], self.position
            );
        }
    }

    fn attribute(&mut self) -> (String, String) {
        let key = self.expect_text();
        self.expect(Token::Equal);
        let value = self.expect_quoted_text();
        (key, value)
    }

    fn attributes(&mut self) -> Vec<(String, String)> {
        let mut attributes = vec![];

        while self.position < self.tokens.len() && self.tokens[self.position] != Token::RAngle {
            attributes.push(self.attribute());
        }

        attributes
    }

    fn element(&mut self) -> HtmlElement {
        if self.tokens[self.position] != Token::LAngle {
            return HtmlElement {
                name: "textNode".to_string(),
                attributes: vec![],
                children: vec![],
                text_node: Some(self.expect_text()),
            };
        }

        self.expect(Token::LAngle);
        let name = self.expect_text();
        let attributes = self.attributes();
        self.expect(Token::RAngle);

        let children: Vec<HtmlElement> = self.elements();

        self.expect(Token::LAngle);
        self.expect(Token::Slash);
        self.expect(Token::Text(name.clone()));
        self.expect(Token::RAngle);

        HtmlElement {
            name,
            attributes,
            children,
            text_node: None,
        }
    }

    fn elements(&mut self) -> Vec<HtmlElement> {
        let mut elements = vec![];

        while self.position < self.tokens.len()
            && !(self.tokens[self.position] == Token::LAngle
                && self.tokens[self.position + 1] == Token::Slash)
        {
            elements.push(self.element());
        }

        elements
    }
}

fn parse_html(str: String) -> HtmlElement {
    println!("Parsing HTML: {}", str);
    let tokens = tokenize_html(str);
    println!("Tokens: {:?}", tokens);
    let mut parser = HtmlParser::new(tokens);
    println!("Element: {:?}", parser.element());
    parser.element()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();

    Ok(())
}
