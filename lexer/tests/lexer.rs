use easyscript::{lex, Token, TokenKind};

fn token(kind: TokenKind, lexeme: &str, line: usize, column: usize) -> Token {
    Token {
        kind,
        lexeme: lexeme.to_owned(),
        line,
        column,
    }
}

#[test]
fn lexes_hello_world_source_with_locations() {
    let source = r#"active system linux;
from library import std;
def HelloWorld() -> int {
    return code -> "Hello, World!";
}
rules! {
    stop;
}
stop;
"#;

    let tokens = lex(source).unwrap();
    assert_eq!(
        tokens,
        vec![
            token(TokenKind::Active, "active", 1, 1),
            token(TokenKind::System, "system", 1, 8),
            token(TokenKind::Linux, "linux", 1, 15),
            token(TokenKind::Semicolon, ";", 1, 20),
            token(TokenKind::From, "from", 2, 1),
            token(TokenKind::Library, "library", 2, 6),
            token(TokenKind::Import, "import", 2, 14),
            token(TokenKind::Std, "std", 2, 21),
            token(TokenKind::Semicolon, ";", 2, 24),
            token(TokenKind::Def, "def", 3, 1),
            token(
                TokenKind::Identifier("HelloWorld".to_owned()),
                "HelloWorld",
                3,
                5
            ),
            token(TokenKind::LeftParen, "(", 3, 15),
            token(TokenKind::RightParen, ")", 3, 16),
            token(TokenKind::Arrow, "->", 3, 18),
            token(TokenKind::Int, "int", 3, 21),
            token(TokenKind::LeftBrace, "{", 3, 25),
            token(TokenKind::Return, "return", 4, 5),
            token(TokenKind::Code, "code", 4, 12),
            token(TokenKind::Arrow, "->", 4, 17),
            token(
                TokenKind::StringLiteral("Hello, World!".to_owned()),
                r#""Hello, World!""#,
                4,
                20
            ),
            token(TokenKind::Semicolon, ";", 4, 35),
            token(TokenKind::RightBrace, "}", 5, 1),
            token(TokenKind::RulesBang, "rules!", 6, 1),
            token(TokenKind::LeftBrace, "{", 6, 8),
            token(TokenKind::Stop, "stop", 7, 5),
            token(TokenKind::Semicolon, ";", 7, 9),
            token(TokenKind::RightBrace, "}", 8, 1),
            token(TokenKind::Stop, "stop", 9, 1),
            token(TokenKind::Semicolon, ";", 9, 5),
        ]
    );
}

#[test]
fn lexes_addition_source() {
    let tokens = lex("A+B").unwrap();
    assert_eq!(
        tokens,
        vec![
            token(TokenKind::Identifier("A".to_owned()), "A", 1, 1),
            token(TokenKind::Plus, "+", 1, 2),
            token(TokenKind::Identifier("B".to_owned()), "B", 1, 3),
        ]
    );
}

#[test]
fn lexes_function_parameters_and_auto_type() {
    let tokens = lex("def sum(a, b) -> auto").unwrap();
    assert_eq!(
        tokens,
        vec![
            token(TokenKind::Def, "def", 1, 1),
            token(TokenKind::Identifier("sum".to_owned()), "sum", 1, 5),
            token(TokenKind::LeftParen, "(", 1, 8),
            token(TokenKind::Identifier("a".to_owned()), "a", 1, 9),
            token(TokenKind::Comma, ",", 1, 10),
            token(TokenKind::Identifier("b".to_owned()), "b", 1, 12),
            token(TokenKind::RightParen, ")", 1, 13),
            token(TokenKind::Arrow, "->", 1, 15),
            token(TokenKind::Auto, "auto", 1, 18),
        ]
    );
}

#[test]
fn lexes_integer_and_arithmetic_operators() {
    let tokens = lex("12-3*4/2").unwrap();
    assert_eq!(
        tokens,
        vec![
            token(TokenKind::Number("12".to_owned()), "12", 1, 1),
            token(TokenKind::Minus, "-", 1, 3),
            token(TokenKind::Number("3".to_owned()), "3", 1, 4),
            token(TokenKind::Star, "*", 1, 5),
            token(TokenKind::Number("4".to_owned()), "4", 1, 6),
            token(TokenKind::Slash, "/", 1, 7),
            token(TokenKind::Number("2".to_owned()), "2", 1, 8),
        ]
    );
}

#[test]
fn lexes_comments_and_escaped_strings() {
    let tokens = lex("# note\n\"line\\n\\\"quoted\\\"\"").unwrap();
    assert_eq!(
        tokens,
        vec![
            token(TokenKind::Comment(" note".to_owned()), "# note", 1, 1),
            token(
                TokenKind::StringLiteral("line\n\"quoted\"".to_owned()),
                r#""line\n\"quoted\"""#,
                2,
                1
            ),
        ]
    );
}

#[test]
fn reports_invalid_characters_at_their_source_location() {
    let error = lex("active\n@").unwrap_err();
    assert_eq!(error.line, 2);
    assert_eq!(error.column, 1);
    assert_eq!(
        error.to_string(),
        "unexpected character '@' at line 2, column 1"
    );
}
