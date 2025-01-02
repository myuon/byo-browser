use std::rc::Rc;

use anyhow::{bail, Context};

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
                && !chars[position].is_whitespace()
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

    fn peek(&self) -> Option<&Token> {
        if self.position < self.tokens.len() {
            Some(&self.tokens[self.position])
        } else {
            None
        }
    }

    fn starts_with(&self, tokens: &[Token]) -> bool {
        self.tokens[self.position..].starts_with(tokens)
    }

    fn expect(&mut self, token: Token) -> Result<(), anyhow::Error> {
        if self.tokens[self.position] != token {
            bail!(
                "Want {:?}, but got {:?} ({})",
                token,
                &self.tokens[self.position..],
                self.position
            );
        }
        self.position += 1;

        Ok(())
    }

    fn expect_text(&mut self) -> Result<String, anyhow::Error> {
        if let Token::Text(text) = &self.tokens[self.position] {
            self.position += 1;
            Ok(text.clone())
        } else {
            bail!(
                "Want text, but got {:?} ({})",
                &self.tokens[self.position..],
                self.position
            );
        }
    }

    fn expect_quoted_text(&mut self) -> Result<String, anyhow::Error> {
        if let Token::QuotedText(text) = &self.tokens[self.position] {
            self.position += 1;
            Ok(text.clone())
        } else {
            bail!(
                "Want quoted text, but got {:?} ({})",
                &self.tokens[self.position..],
                self.position
            );
        }
    }

    fn attribute(&mut self) -> Result<(String, String), anyhow::Error> {
        let key = self.expect_text()?;
        self.expect(Token::Equal)?;
        let value = self.expect_quoted_text()?;
        Ok((key, value))
    }

    fn attributes(&mut self) -> Result<Vec<(String, String)>, anyhow::Error> {
        let mut attributes = vec![];

        while self.peek() != Some(&Token::RAngle) && self.peek() != Some(&Token::Slash) {
            attributes.push(self.attribute()?);
        }

        Ok(attributes)
    }

    fn element(&mut self) -> Result<HtmlElement, anyhow::Error> {
        if matches!(self.peek(), Some(Token::Text(_))) {
            return Ok(HtmlElement {
                name: "textNode".to_string(),
                attributes: vec![],
                children: vec![],
                text_node: Some(self.expect_text()?),
            });
        }

        self.expect(Token::LAngle)?;
        let name = self.expect_text()?;
        let attributes = self.attributes()?;
        if self.peek() == Some(&Token::Slash) {
            self.expect(Token::Slash)?;
            self.expect(Token::RAngle)?;
            return Ok(HtmlElement {
                name,
                attributes,
                children: vec![],
                text_node: None,
            });
        }
        self.expect(Token::RAngle)?;

        let children: Vec<HtmlElement> =
            self.elements().context(format!("children of {}", name))?;

        self.expect(Token::LAngle)?;
        self.expect(Token::Slash)?;
        self.expect(Token::Text(name.clone()))?;
        self.expect(Token::RAngle)?;

        Ok(HtmlElement {
            name,
            attributes,
            children,
            text_node: None,
        })
    }

    fn elements(&mut self) -> Result<Vec<HtmlElement>, anyhow::Error> {
        let mut elements = vec![];

        while self.position < self.tokens.len() && !self.starts_with(&[Token::LAngle, Token::Slash])
        {
            elements.push(self.element().context(format!(
                "element with {:?}",
                &self.tokens[self.position..(self.position + 5).min(self.tokens.len())]
            ))?);
        }

        Ok(elements)
    }
}

pub fn parse_html(str: String) -> Result<HtmlElement, anyhow::Error> {
    println!("Parsing HTML: {}", str);
    let tokens = tokenize_html(str);
    println!("Tokens: {:?}", tokens);
    let mut parser = HtmlParser::new(tokens);
    let element = parser.element()?;
    println!("Element: {:?}", element);

    Ok(element)
}

#[test]
fn test_parse_html() {
    use pretty_assertions::assert_eq;

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
            children: vec![
                HtmlElement {
                    name: "head".to_string(),
                    attributes: vec![],
                    children: vec![HtmlElement {
                        name: "title".to_string(),
                        attributes: vec![],
                        children: vec![
                            HtmlElement {
                                name: "textNode".to_string(),
                                attributes: vec![],
                                children: vec![],
                                text_node: Some("sample".to_string()),
                            },
                            HtmlElement {
                                name: "textNode".to_string(),
                                attributes: vec![],
                                children: vec![],
                                text_node: Some("web".to_string()),
                            },
                            HtmlElement {
                                name: "textNode".to_string(),
                                attributes: vec![],
                                children: vec![],
                                text_node: Some("page".to_string()),
                            },
                        ],
                        text_node: None,
                    }],
                    text_node: None,
                },
                HtmlElement {
                    name: "body".to_string(),
                    attributes: vec![
                        ("bgcolor".to_string(), "#cccccc".to_string()),
                        ("text".to_string(), "#ffffff".to_string()),
                    ],
                    children: vec![
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("Hello,".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("world!".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("This".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("is".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("a".to_string()),
                        },
                        HtmlElement {
                            name: "a".to_string(),
                            attributes: vec![("href".to_string(), "link1.html".to_string())],
                            children: vec![HtmlElement {
                                name: "textNode".to_string(),
                                attributes: vec![],
                                children: vec![],
                                text_node: Some("link".to_string()),
                            }],
                            text_node: None,
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some(".".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("This".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("is".to_string()),
                        },
                        HtmlElement {
                            name: "br".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: None,
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("a".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("new".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("line.".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("And".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("a".to_string()),
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some("new".to_string()),
                        },
                        HtmlElement {
                            name: "a".to_string(),
                            attributes: vec![("href".to_string(), "link2.html".to_string())],
                            children: vec![HtmlElement {
                                name: "textNode".to_string(),
                                attributes: vec![],
                                children: vec![],
                                text_node: Some("link".to_string()),
                            }],
                            text_node: None,
                        },
                        HtmlElement {
                            name: "textNode".to_string(),
                            attributes: vec![],
                            children: vec![],
                            text_node: Some(".".to_string()),
                        },
                    ],
                    text_node: None,
                },
            ],
            text_node: None,
        },
    )];

    for (str, want) in cases {
        assert_eq!(parse_html(str.to_string()).unwrap(), want);
    }
}
