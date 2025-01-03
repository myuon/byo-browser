use anyhow::bail;

#[derive(Debug, PartialEq, Eq, Clone)]
enum Token {
    LBrace,
    RBrace,
    Colon,
    SemiColon,
    Ident(String),
}

fn tokenize_css(str: String) -> Vec<Token> {
    let mut tokens = vec![];
    let chars = str.chars().collect::<Vec<_>>();
    let mut position = 0;

    while position < chars.len() {
        if chars[position].is_whitespace() {
            position += 1;
            continue;
        } else if chars[position] == '{' {
            tokens.push(Token::LBrace);
            position += 1;
        } else if chars[position] == '}' {
            tokens.push(Token::RBrace);
            position += 1;
        } else if chars[position] == ':' {
            tokens.push(Token::Colon);
            position += 1;
        } else if chars[position] == ';' {
            tokens.push(Token::SemiColon);
            position += 1;
        } else {
            let mut text = String::new();
            text.push(chars[position]);
            position += 1;
            while position < chars.len()
                && !chars[position].is_whitespace()
                && chars[position] != '{'
                && chars[position] != '}'
                && chars[position] != ':'
                && chars[position] != ';'
            {
                text.push(chars[position]);
                position += 1;
            }
            tokens.push(Token::Ident(text));
        }
    }

    tokens
}

#[derive(Debug)]
pub struct Styles {
    pub styles: Vec<Style>,
}

#[derive(Debug)]
pub struct Style {
    pub selector: Option<String>,
    pub rules: Vec<(String, String)>,
}

struct CssParser {
    tokens: Vec<Token>,
    position: usize,
}

impl CssParser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
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

    fn expect_ident(&mut self) -> Result<String, anyhow::Error> {
        if let Token::Ident(text) = &self.tokens[self.position] {
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

    fn styles(&mut self) -> Result<Styles, anyhow::Error> {
        let mut styles = vec![];

        while self.position < self.tokens.len() {
            styles.push(self.style()?);
        }

        Ok(Styles { styles })
    }

    fn style(&mut self) -> Result<Style, anyhow::Error> {
        let prev_position = self.position;
        let ident = self.expect_ident()?;
        match self.peek() {
            Some(Token::LBrace) => {
                self.expect(Token::LBrace)?;
                let rules = self.rules()?;
                self.expect(Token::RBrace)?;
                Ok(Style {
                    selector: Some(ident),
                    rules,
                })
            }
            Some(Token::Colon) => {
                self.position = prev_position;
                let rules = self.rules()?;
                Ok(Style {
                    selector: None,
                    rules,
                })
            }
            _ => bail!("Unexpected token: {:?}", self.peek()),
        }
    }

    fn rules(&mut self) -> Result<Vec<(String, String)>, anyhow::Error> {
        let mut rules = vec![];

        while let Some(Token::Ident(ident)) = self.peek() {
            let ident = self.expect_ident()?;
            self.expect(Token::Colon)?;
            let value = self.expect_ident()?;
            self.expect(Token::SemiColon)?;
            rules.push((ident, value));
        }

        Ok(rules)
    }
}

pub fn parse_css(str: String) -> Result<Styles, anyhow::Error> {
    println!("Parsing CSS: {}", str);
    let tokens = tokenize_css(str);
    println!("Tokens: {:?}", tokens);
    let mut parser = CssParser::new(tokens);
    let element = parser.styles()?;
    println!("Element: {:?}", element);

    Ok(element)
}
