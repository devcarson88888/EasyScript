use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Active,
    System,
    Linux,
    From,
    Library,
    Import,
    Std,
    Def,
    Fn,
    Struct,
    Enum,
    Extern,
    If,
    Elif,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Try,
    Except,
    Finally,
    Class,
    Const,
    Type,
    Lambda,
    Espkg,
    Package,
    Thread,
    Spawn,
    Join,
    GcBang,
    True,
    False,
    Return,
    Code,
    RulesBang,
    Stop,
    Int,
    Auto,
    Identifier(String),
    Number(String),
    StringLiteral(String),
    Comment(String),
    Arrow,
    Assign,
    Ge,
    Gt,
    Le,
    Lt,
    EqEq,
    BangEq,
    Not,
    And,
    Or,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    LeftParen,
    RightParen,
    Comma,
    Dot,
    Colon,
    Semicolon,
    Hash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for LexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at line {}, column {}",
            self.message, self.line, self.column
        )
    }
}

impl Error for LexError {}

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(source).scan_tokens()
}

struct Lexer<'source> {
    source: &'source str,
    offset: usize,
    line: usize,
    column: usize,
}

impl<'source> Lexer<'source> {
    fn new(source: &'source str) -> Self {
        Self {
            source,
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    fn scan_tokens(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        while let Some(character) = self.peek() {
            if character.is_whitespace() {
                self.advance();
                continue;
            }

            let start = self.offset;
            let line = self.line;
            let column = self.column;
            let kind = match character {
                '#' if self.peek_next() == Some('[') => {
                    self.advance();
                    TokenKind::Hash
                }
                '#' => self.scan_comment(),
                '/' if self.peek_next() == Some('*') => self.scan_block_comment(line, column)?,
                '"' => self.scan_string()?,
                '0'..='9' => self.scan_number(),
                'a'..='z' | 'A'..='Z' | '_' => self.scan_identifier(),
                '-' if self.peek_next() == Some('>') => {
                    self.advance();
                    self.advance();
                    TokenKind::Arrow
                }
                '>' if self.peek_next() == Some('=') => {
                    self.advance();
                    self.advance();
                    TokenKind::Ge
                }
                '>' => {
                    self.advance();
                    TokenKind::Gt
                }
                '<' if self.peek_next() == Some('=') => {
                    self.advance();
                    self.advance();
                    TokenKind::Le
                }
                '<' => {
                    self.advance();
                    TokenKind::Lt
                }
                '=' if self.peek_next() == Some('=') => {
                    self.advance();
                    self.advance();
                    TokenKind::EqEq
                }
                '=' => {
                    self.advance();
                    TokenKind::Assign
                }
                '!' if self.peek_next() == Some('=') => {
                    self.advance();
                    self.advance();
                    TokenKind::BangEq
                }
                '!' => {
                    self.advance();
                    TokenKind::Not
                }
                '&' if self.peek_next() == Some('&') => {
                    self.advance();
                    self.advance();
                    TokenKind::And
                }
                '|' if self.peek_next() == Some('|') => {
                    self.advance();
                    self.advance();
                    TokenKind::Or
                }
                '+' => {
                    self.advance();
                    TokenKind::Plus
                }
                '-' => {
                    self.advance();
                    TokenKind::Minus
                }
                '*' => {
                    self.advance();
                    TokenKind::Star
                }
                '/' => {
                    self.advance();
                    TokenKind::Slash
                }
                '%' => {
                    self.advance();
                    TokenKind::Percent
                }
                '{' => {
                    self.advance();
                    TokenKind::LeftBrace
                }
                '}' => {
                    self.advance();
                    TokenKind::RightBrace
                }
                '[' => {
                    self.advance();
                    TokenKind::LeftBracket
                }
                ']' => {
                    self.advance();
                    TokenKind::RightBracket
                }
                '(' => {
                    self.advance();
                    TokenKind::LeftParen
                }
                ')' => {
                    self.advance();
                    TokenKind::RightParen
                }
                ',' => {
                    self.advance();
                    TokenKind::Comma
                }
                '.' => {
                    self.advance();
                    TokenKind::Dot
                }
                ':' => {
                    self.advance();
                    TokenKind::Colon
                }
                ';' => {
                    self.advance();
                    TokenKind::Semicolon
                }
                _ => {
                    return Err(LexError {
                        message: format!("unexpected character {character:?}"),
                        line,
                        column,
                    });
                }
            };

            tokens.push(Token {
                kind,
                lexeme: self.source[start..self.offset].to_owned(),
                line,
                column,
            });
        }

        Ok(tokens)
    }

    fn scan_comment(&mut self) -> TokenKind {
        self.advance();
        let content_start = self.offset;
        while !matches!(self.peek(), None | Some('\n' | '\r')) {
            self.advance();
        }
        TokenKind::Comment(self.source[content_start..self.offset].to_owned())
    }

    fn scan_block_comment(&mut self, line: usize, column: usize) -> Result<TokenKind, LexError> {
        self.advance();
        self.advance();
        let content_start = self.offset;
        while !matches!(self.peek(), None)
            && !(self.peek() == Some('*') && self.peek_next() == Some('/'))
        {
            self.advance();
        }
        if self.peek().is_none() {
            return Err(LexError {
                message: "unterminated block comment".to_owned(),
                line,
                column,
            });
        }
        let content_end = self.offset;
        self.advance();
        self.advance();
        Ok(TokenKind::Comment(
            self.source[content_start..content_end].to_owned(),
        ))
    }

    fn scan_string(&mut self) -> Result<TokenKind, LexError> {
        let line = self.line;
        let column = self.column;
        self.advance();

        let mut value = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(LexError {
                        message: "unterminated string literal".to_owned(),
                        line,
                        column,
                    });
                }
                Some('"') => {
                    self.advance();
                    return Ok(TokenKind::StringLiteral(value));
                }
                Some('\\') => {
                    self.advance();
                    let escape_line = self.line;
                    let escape_column = self.column;
                    let escaped = self.advance().ok_or_else(|| LexError {
                        message: "unterminated escape sequence".to_owned(),
                        line: escape_line,
                        column: escape_column,
                    })?;
                    match escaped {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        _ => {
                            return Err(LexError {
                                message: format!("unsupported escape sequence \\{escaped}"),
                                line: escape_line,
                                column: escape_column,
                            });
                        }
                    }
                }
                Some('\n') => {
                    return Err(LexError {
                        message: "newline in string literal; use \\n for a newline".to_owned(),
                        line: self.line,
                        column: self.column,
                    });
                }
                Some(character) => {
                    self.advance();
                    value.push(character);
                }
            }
        }
    }

    fn scan_number(&mut self) -> TokenKind {
        let start = self.offset;
        while matches!(self.peek(), Some('0'..='9')) {
            self.advance();
        }
        TokenKind::Number(self.source[start..self.offset].to_owned())
    }

    fn scan_identifier(&mut self) -> TokenKind {
        let start = self.offset;
        self.advance();
        while matches!(self.peek(), Some('a'..='z' | 'A'..='Z' | '0'..='9' | '_')) {
            self.advance();
        }
        let identifier_end = self.offset;
        let identifier = &self.source[start..identifier_end];
        let bang_token = match (identifier, self.peek()) {
            ("rules", Some('!')) => Some(TokenKind::RulesBang),
            ("gc", Some('!')) => Some(TokenKind::GcBang),
            _ => None,
        };
        if bang_token.is_some() {
            self.advance();
            return bang_token.expect("checked for bang token");
        }

        match identifier {
            "active" => TokenKind::Active,
            "system" => TokenKind::System,
            "linux" => TokenKind::Linux,
            "from" => TokenKind::From,
            "library" => TokenKind::Library,
            "import" => TokenKind::Import,
            "std" => TokenKind::Std,
            "def" => TokenKind::Def,
            "fn" => TokenKind::Fn,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "extern" => TokenKind::Extern,
            "if" => TokenKind::If,
            "elif" => TokenKind::Elif,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "try" => TokenKind::Try,
            "except" => TokenKind::Except,
            "finally" => TokenKind::Finally,
            "class" => TokenKind::Class,
            "const" => TokenKind::Const,
            "type" => TokenKind::Type,
            "lambda" => TokenKind::Lambda,
            "espkg" => TokenKind::Espkg,
            "package" => TokenKind::Package,
            "thread" => TokenKind::Thread,
            "spawn" => TokenKind::Spawn,
            "join" => TokenKind::Join,
            "True" => TokenKind::True,
            "False" => TokenKind::False,
            "return" => TokenKind::Return,
            "code" => TokenKind::Code,
            "stop" => TokenKind::Stop,
            "int" => TokenKind::Int,
            "auto" => TokenKind::Auto,
            _ => TokenKind::Identifier(identifier.to_owned()),
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let mut characters = self.source[self.offset..].chars();
        characters.next()?;
        characters.next()
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(character)
    }
}
