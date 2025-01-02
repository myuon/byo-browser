use std::rc::Rc;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Token {
    LAngle,
    RAngle,
    Slash,
    Equal,
    Text(String),
    QuotedText(String),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HtmlElement {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<HtmlElement>,
    pub text_node: Option<String>,
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
                &self.tokens[self.position..],
                self.position
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
                &self.tokens[self.position..],
                self.position
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
                &self.tokens[self.position..],
                self.position
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
        if self.tokens[self.position] == Token::Slash {
            self.expect(Token::Slash);
            self.expect(Token::RAngle);
            return HtmlElement {
                name,
                attributes,
                children: vec![],
                text_node: None,
            };
        }
        self.expect(Token::RAngle);

        let children: Vec<HtmlElement> = if self.tokens[self.position] == Token::LAngle
            && self.tokens[self.position + 1] == Token::Slash
        {
            vec![]
        } else {
            self.elements()
        };

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

pub fn parse_html(str: String) -> HtmlElement {
    println!("Parsing HTML: {}", str);
    let tokens = tokenize_html(str);
    println!("Tokens: {:?}", tokens);
    let mut parser = HtmlParser::new(tokens);
    println!("Element: {:?}", parser.element());
    parser.element()
}

#[test]
fn test_parse_html() {
    let cases = vec![(
        r##"<html>
  <head>
    <title>sample web page</title>
  </head>
  <body bgcolor="#cccccc" text="#ffffff">
    Hello, world! This is a <a href="link1.html">link</a>. This is <br /> a new line. And a new <a href="link2.html">link</a>.
  </body>
</html>
"##,
        HtmlElement {
            name: "html".to_string(),
            attributes: vec![],
            children: vec![HtmlElement {
                name: "head".to_string(),
                attributes: vec![],
                children: vec![HtmlElement {
                    name: "title".to_string(),
                    attributes: vec![],
                    children: vec![],
                    text_node: Some("sample web page".to_string()),
                }],
                text_node: None,
            }],
            text_node: None,
        },
    )];

    for (str, want) in cases {
        assert_eq!(parse_html(str.to_string()), want);
    }
}
