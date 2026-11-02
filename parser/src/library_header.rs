use crate::lexer::{lex, Token, TokenKind};

pub fn parse_library_header(source: &str) -> Result<Vec<(String, String)>, String> {
    let source = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let tokens = lex(&source).map_err(|error| error.to_string())?;
    let tokens: Vec<_> = tokens
        .into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Comment(_)))
        .collect();
    let mut cursor = 0;
    expect_identifier(&tokens, &mut cursor, "namespace")?;
    expect(&tokens, &mut cursor, &TokenKind::Std, "`std` namespace")?;
    expect(
        &tokens,
        &mut cursor,
        &TokenKind::LeftBrace,
        "`{` after namespace",
    )?;

    let mut declarations = Vec::new();
    while !matches!(
        tokens.get(cursor).map(|token| &token.kind),
        Some(TokenKind::RightBrace)
    ) {
        expect(
            &tokens,
            &mut cursor,
            &TokenKind::Extern,
            "`extern` declaration",
        )?;
        expect(
            &tokens,
            &mut cursor,
            &TokenKind::Std,
            "`std` extern namespace",
        )?;
        let name = expect_any_identifier(&tokens, &mut cursor, "extern symbol name")?;
        expect(
            &tokens,
            &mut cursor,
            &TokenKind::Arrow,
            "`->` before ABI symbol",
        )?;
        let abi = expect_any_identifier(&tokens, &mut cursor, "ABI symbol name")?;
        declarations.push((name, abi));
    }

    expect(
        &tokens,
        &mut cursor,
        &TokenKind::RightBrace,
        "`}` after declarations",
    )?;
    if cursor != tokens.len() {
        return Err(format!(
            "unexpected token `{}` after std library header",
            tokens[cursor].lexeme
        ));
    }
    if declarations.is_empty() {
        return Err("std library header has no extern declarations".to_owned());
    }
    Ok(declarations)
}

fn expect_identifier(tokens: &[Token], cursor: &mut usize, expected: &str) -> Result<(), String> {
    match tokens.get(*cursor).map(|token| &token.kind) {
        Some(TokenKind::Identifier(identifier)) if identifier == expected => {
            *cursor += 1;
            Ok(())
        }
        _ => Err(format!("expected `{expected}` in std library header")),
    }
}

fn expect_any_identifier(
    tokens: &[Token],
    cursor: &mut usize,
    description: &str,
) -> Result<String, String> {
    match tokens.get(*cursor).map(|token| &token.kind) {
        Some(TokenKind::Identifier(identifier)) => {
            *cursor += 1;
            Ok(identifier.clone())
        }
        _ => Err(format!("expected {description} in std library header")),
    }
}

fn expect(
    tokens: &[Token],
    cursor: &mut usize,
    expected: &TokenKind,
    description: &str,
) -> Result<(), String> {
    if tokens
        .get(*cursor)
        .is_some_and(|token| token.kind == *expected)
    {
        *cursor += 1;
        Ok(())
    } else {
        Err(format!("expected {description} in std library header"))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_library_header;

    #[test]
    fn parses_header_symbol_to_abi_mappings() {
        let declarations = parse_library_header(
            "// header\nnamespace std {\n    extern std write -> es_std_write\n    extern std input -> es_std_input\n    extern std output -> es_std_output\n}\n",
        )
        .unwrap();
        assert_eq!(
            declarations,
            [
                ("write".to_owned(), "es_std_write".to_owned()),
                ("input".to_owned(), "es_std_input".to_owned()),
                ("output".to_owned(), "es_std_output".to_owned()),
            ]
        );
    }

    #[test]
    fn rejects_implementation_tokens_in_header() {
        assert!(
            parse_library_header("namespace std {\n    def println() {}\n}\n")
                .unwrap_err()
                .contains("expected `extern`")
        );
    }
}
