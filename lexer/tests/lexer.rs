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
fn lexes_container_brackets_and_member_access_punctuation() {
    let tokens = lex("[] [0] arr.append(1) (1, 2)").unwrap();
    assert_eq!(
        tokens,
        vec![
            token(TokenKind::LeftBracket, "[", 1, 1),
            token(TokenKind::RightBracket, "]", 1, 2),
            token(TokenKind::LeftBracket, "[", 1, 4),
            token(TokenKind::Number("0".to_owned()), "0", 1, 5),
            token(TokenKind::RightBracket, "]", 1, 6),
            token(TokenKind::Identifier("arr".to_owned()), "arr", 1, 8),
            token(TokenKind::Dot, ".", 1, 11),
            token(TokenKind::Identifier("append".to_owned()), "append", 1, 12),
            token(TokenKind::LeftParen, "(", 1, 18),
            token(TokenKind::Number("1".to_owned()), "1", 1, 19),
            token(TokenKind::RightParen, ")", 1, 20),
            token(TokenKind::LeftParen, "(", 1, 22),
            token(TokenKind::Number("1".to_owned()), "1", 1, 23),
            token(TokenKind::Comma, ",", 1, 24),
            token(TokenKind::Number("2".to_owned()), "2", 1, 26),
            token(TokenKind::RightParen, ")", 1, 27),
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
fn lexes_keywords_and_multi_character_operators_without_splitting() {
    let tokens =
        lex("if elif else while break continue True False >= > <= < == != ! && || = ->").unwrap();
    assert_eq!(
        tokens,
        vec![
            token(TokenKind::If, "if", 1, 1),
            token(TokenKind::Elif, "elif", 1, 4),
            token(TokenKind::Else, "else", 1, 9),
            token(TokenKind::While, "while", 1, 14),
            token(TokenKind::Break, "break", 1, 20),
            token(TokenKind::Continue, "continue", 1, 26),
            token(TokenKind::True, "True", 1, 35),
            token(TokenKind::False, "False", 1, 40),
            token(TokenKind::Ge, ">=", 1, 46),
            token(TokenKind::Gt, ">", 1, 49),
            token(TokenKind::Le, "<=", 1, 51),
            token(TokenKind::Lt, "<", 1, 54),
            token(TokenKind::EqEq, "==", 1, 56),
            token(TokenKind::BangEq, "!=", 1, 59),
            token(TokenKind::Not, "!", 1, 62),
            token(TokenKind::And, "&&", 1, 64),
            token(TokenKind::Or, "||", 1, 67),
            token(TokenKind::Assign, "=", 1, 70),
            token(TokenKind::Arrow, "->", 1, 72),
        ]
    );
}

#[test]
fn lexes_rules_commands_and_final_stop_marker() {
    let tokens = lex("rules! write -> value; rules! output -> 2; stop;").unwrap();
    assert_eq!(
        tokens
            .iter()
            .map(|token| token.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            TokenKind::RulesBang,
            TokenKind::Identifier("write".to_owned()),
            TokenKind::Arrow,
            TokenKind::Identifier("value".to_owned()),
            TokenKind::Semicolon,
            TokenKind::RulesBang,
            TokenKind::Identifier("output".to_owned()),
            TokenKind::Arrow,
            TokenKind::Number("2".to_owned()),
            TokenKind::Semicolon,
            TokenKind::Stop,
            TokenKind::Semicolon,
        ]
    );
    assert_eq!((tokens[0].line, tokens[0].column), (1, 1));
    assert_eq!((tokens[5].line, tokens[5].column), (1, 24));
}

#[test]
fn lexes_package_gc_and_thread_keywords() {
    let tokens = lex("espkg package gc! collect thread spawn join").unwrap();
    assert_eq!(
        tokens
            .iter()
            .map(|token| token.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            TokenKind::Espkg,
            TokenKind::Package,
            TokenKind::GcBang,
            TokenKind::Identifier("collect".to_owned()),
            TokenKind::Thread,
            TokenKind::Spawn,
            TokenKind::Join,
        ]
    );
    assert_eq!(tokens[2].lexeme, "gc!");
    assert_eq!((tokens[2].line, tokens[2].column), (1, 15));
}

#[test]
fn lexes_fourth_tier_declaration_keywords_and_aot_marker() {
    let tokens = lex("fn struct enum extern #[aot]").unwrap();
    assert_eq!(
        tokens
            .iter()
            .map(|token| token.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            TokenKind::Fn,
            TokenKind::Struct,
            TokenKind::Enum,
            TokenKind::Extern,
            TokenKind::Hash,
            TokenKind::LeftBracket,
            TokenKind::Identifier("aot".to_owned()),
            TokenKind::RightBracket,
        ]
    );
    assert_eq!((tokens[4].line, tokens[4].column), (1, 23));
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
