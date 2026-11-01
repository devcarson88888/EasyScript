use std::error::Error;
use std::fmt;

use crate::ast::{
    AssignmentStatement, AssignmentTarget, AssignmentTargetKind, BinaryOperator, Block,
    BreakStatement, ClassDefinition, ClassMember, ConstStatement, ContinueStatement,
    EmptyStatement, EnumDefinition, EnumVariant, EnumVariantPayload, ExceptHandler, Expr, ExprKind,
    ExpressionStatement, ForStatement, Function, GcControlStatement, GcOperation, GenericParameter,
    HeaderDeclaration, IfStatement, ImportDeclaration, JoinStatement, Parameter, Program,
    ReturnStatement, RulesStatement, SpawnStatement, Statement, StructDefinition, StructField,
    ThreadStatement, TopLevelItem, TryStatement, TypeName, UnaryOperator, WhileStatement,
};
use crate::lexer::{lex, LexError, Token, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at line {}, column {}",
            self.message, self.line, self.column
        )
    }
}

impl Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(error: LexError) -> Self {
        Self {
            message: error.message,
            line: error.line,
            column: error.column,
        }
    }
}

pub fn parse(source: &str) -> Result<Program, ParseError> {
    let tokens = lex(source)?;
    Parser::new(tokens, Some(source_end_position(source))).parse_program()
}

pub fn parse_tokens(tokens: Vec<Token>) -> Result<Program, ParseError> {
    Parser::new(tokens, None).parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    current: usize,
    loop_depth: usize,
    eof_line: usize,
    eof_column: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>, source_end: Option<(usize, usize)>) -> Self {
        let tokens: Vec<_> = tokens
            .into_iter()
            .filter(|token| !matches!(token.kind, TokenKind::Comment(_)))
            .collect();
        let (eof_line, eof_column) = source_end.unwrap_or_else(|| {
            tokens.last().map_or((1, 1), |token| {
                (token.line, token.column + token.lexeme.chars().count())
            })
        });
        Self {
            tokens,
            current: 0,
            loop_depth: 0,
            eof_line,
            eof_column,
        }
    }

    fn parse_program(mut self) -> Result<Program, ParseError> {
        let (line, column) = self.position();
        let mut items = Vec::new();
        let mut entry_count = 0;

        while !self.is_at_end() {
            let item = match self.peek_kind() {
                Some(TokenKind::Active) => TopLevelItem::Header(self.parse_header()?),
                Some(TokenKind::From) => TopLevelItem::Import(self.parse_import()?),
                Some(TokenKind::Def) => TopLevelItem::Function(self.parse_function()?),
                Some(TokenKind::Int) => {
                    let function = self.parse_entry_function()?;
                    entry_count += 1;
                    if entry_count > 1 {
                        return Err(ParseError {
                            message: "the program may contain only one `int main()` entry point"
                                .to_owned(),
                            line: function.line,
                            column: function.column,
                        });
                    }
                    TopLevelItem::Function(function)
                }
                Some(TokenKind::Struct) => TopLevelItem::Struct(self.parse_struct()?),
                Some(TokenKind::Enum) => TopLevelItem::Enum(self.parse_enum()?),
                Some(TokenKind::Class) => TopLevelItem::Class(self.parse_class()?),
                Some(TokenKind::Stop) => {
                    if entry_count == 0 {
                        return Err(self.error("`stop;` must follow the `int main()` entry point"));
                    }
                    let statement = self.parse_stop_declaration()?;
                    if !self.is_at_end() {
                        return Err(self.error("`stop;` must be the final source marker"));
                    }
                    TopLevelItem::Stop(statement)
                }
                _ => {
                    return Err(self.error(
                        "only `active system`, `from library import`, `def`, `struct`, `enum`, `class`, `int main()`, and the final `stop;` are allowed at top level",
                    ))
                }
            };
            items.push(item);
        }

        if entry_count == 0 {
            return Err(self.error("program is missing its required `int main()` entry point"));
        }

        Ok(Program {
            line,
            column,
            items,
        })
    }

    fn parse_stop_declaration(&mut self) -> Result<crate::ast::StopStatement, ParseError> {
        let start = self.expect(&TokenKind::Stop, "`stop`")?;
        self.expect(&TokenKind::Semicolon, "`;` after the final `stop` marker")?;
        Ok(crate::ast::StopStatement {
            line: start.line,
            column: start.column,
        })
    }

    fn parse_header(&mut self) -> Result<HeaderDeclaration, ParseError> {
        let start = self.expect(&TokenKind::Active, "active")?;
        self.expect(&TokenKind::System, "system after `active`")?;
        let system_token = self
            .advance()
            .ok_or_else(|| self.error("expected a system name after `active system`"))?;
        let system = match system_token.kind {
            TokenKind::Identifier(name) => name,
            TokenKind::Linux => "linux".to_owned(),
            _ => {
                return Err(self.error_at(
                    &system_token,
                    "expected a system name after `active system`",
                ))
            }
        };
        self.expect(&TokenKind::Semicolon, "`;` after the system declaration")?;
        Ok(HeaderDeclaration {
            line: start.line,
            column: start.column,
            system,
        })
    }

    fn parse_import(&mut self) -> Result<ImportDeclaration, ParseError> {
        let start = self.expect(&TokenKind::From, "from")?;
        self.expect(&TokenKind::Library, "`library` after `from`")?;
        self.expect(&TokenKind::Import, "`import` after `from library`")?;
        let library_token = self.advance().ok_or_else(|| {
            self.error("expected a library module name after `from library import`")
        })?;
        let library = match library_token.kind {
            TokenKind::Identifier(name) => name,
            TokenKind::Std => "std".to_owned(),
            _ => {
                return Err(self.error_at(
                    &library_token,
                    "expected a library module name after `from library import`",
                ))
            }
        };
        self.expect(&TokenKind::Semicolon, "`;` after the library import")?;
        Ok(ImportDeclaration {
            line: start.line,
            column: start.column,
            library,
        })
    }

    fn parse_function(&mut self) -> Result<Function, ParseError> {
        let start = self.expect(&TokenKind::Def, "`def`")?;
        let name = self.expect_identifier("a function name")?;
        let parameters = self.parse_parameters()?;
        let return_type = if self.matches(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        self.matches(&TokenKind::Semicolon);

        Ok(Function {
            line: start.line,
            column: start.column,
            name,
            generic_parameters: Vec::new(),
            annotations: Vec::new(),
            return_type,
            parameters,
            body,
            is_entry: false,
        })
    }

    fn parse_generic_parameters(&mut self) -> Result<Vec<GenericParameter>, ParseError> {
        if !self.matches(&TokenKind::Lt) {
            return Ok(Vec::new());
        }
        let mut parameters = Vec::new();
        loop {
            let token = self.expect_identifier_token("a generic parameter name")?;
            let TokenKind::Identifier(name) = token.kind else {
                unreachable!("identifier token was checked")
            };
            parameters.push(GenericParameter {
                line: token.line,
                column: token.column,
                name,
            });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::Gt, "`>` after generic parameters")?;
        Ok(parameters)
    }

    fn parse_struct(&mut self) -> Result<StructDefinition, ParseError> {
        let start = self.expect(&TokenKind::Struct, "`struct`")?;
        let name = self.expect_identifier("a structure name")?;
        let generic_parameters = self.parse_generic_parameters()?;
        self.expect(&TokenKind::LeftBrace, "`{` before structure fields")?;
        let mut fields = Vec::new();
        while !self.is_at_end() && !self.check(&TokenKind::RightBrace) {
            if self.matches(&TokenKind::Semicolon) {
                continue;
            }
            let token = self.expect_identifier_token("a structure field name")?;
            let TokenKind::Identifier(field_name) = token.kind else {
                unreachable!("identifier token was checked")
            };
            self.expect(&TokenKind::Colon, "`:` after a structure field name")?;
            let ty = self.parse_type()?;
            fields.push(StructField {
                line: token.line,
                column: token.column,
                name: field_name,
                ty,
            });
            self.expect(&TokenKind::Semicolon, "`;` after the structure field")?;
        }
        self.expect(&TokenKind::RightBrace, "`}` after structure fields")?;
        Ok(StructDefinition {
            line: start.line,
            column: start.column,
            name,
            generic_parameters,
            fields,
        })
    }

    fn parse_enum(&mut self) -> Result<EnumDefinition, ParseError> {
        let start = self.expect(&TokenKind::Enum, "`enum`")?;
        let name = self.expect_identifier("an enum name")?;
        let generic_parameters = self.parse_generic_parameters()?;
        self.expect(&TokenKind::LeftBrace, "`{` before enum variants")?;
        let mut variants = Vec::new();
        while !self.is_at_end() && !self.check(&TokenKind::RightBrace) {
            if self.matches(&TokenKind::Comma) || self.matches(&TokenKind::Semicolon) {
                continue;
            }
            let token = self.expect_identifier_token("an enum variant name")?;
            let TokenKind::Identifier(variant_name) = token.kind else {
                unreachable!("identifier token was checked")
            };
            let payload = if self.matches(&TokenKind::LeftParen) {
                let mut types = Vec::new();
                if !self.check(&TokenKind::RightParen) {
                    loop {
                        types.push(self.parse_type()?);
                        if !self.matches(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RightParen, "`)` after enum tuple payload")?;
                EnumVariantPayload::Tuple(types)
            } else if self.matches(&TokenKind::LeftBrace) {
                let mut fields = Vec::new();
                while !self.is_at_end() && !self.check(&TokenKind::RightBrace) {
                    if self.matches(&TokenKind::Comma) || self.matches(&TokenKind::Semicolon) {
                        continue;
                    }
                    let field = self.expect_identifier_token("an enum payload field name")?;
                    let TokenKind::Identifier(field_name) = field.kind else {
                        unreachable!("identifier token was checked")
                    };
                    self.expect(&TokenKind::Colon, "`:` after an enum payload field")?;
                    fields.push(StructField {
                        line: field.line,
                        column: field.column,
                        name: field_name,
                        ty: self.parse_type()?,
                    });
                    if !self.matches(&TokenKind::Comma) {
                        self.matches(&TokenKind::Semicolon);
                    }
                }
                self.expect(&TokenKind::RightBrace, "`}` after named enum payload")?;
                EnumVariantPayload::Struct(fields)
            } else {
                EnumVariantPayload::Unit
            };
            variants.push(EnumVariant {
                line: token.line,
                column: token.column,
                name: variant_name,
                payload,
            });
            if !self.matches(&TokenKind::Comma)
                && !self.matches(&TokenKind::Semicolon)
                && !self.check(&TokenKind::RightBrace)
            {
                return Err(self.error("expected `,` or `;` after an enum variant"));
            }
        }
        self.expect(&TokenKind::RightBrace, "`}` after enum variants")?;
        Ok(EnumDefinition {
            line: start.line,
            column: start.column,
            name,
            generic_parameters,
            variants,
        })
    }

    fn parse_entry_function(&mut self) -> Result<Function, ParseError> {
        let start = self.expect(&TokenKind::Int, "`int` for the program entry")?;
        let name_token = self.expect_identifier_token("`main` after `int`")?;
        let name = match name_token.kind {
            TokenKind::Identifier(name) if name == "main" => name,
            _ => {
                return Err(self.error_at(&name_token, "expected `main` after the `int` entry type"))
            }
        };
        self.expect(&TokenKind::LeftParen, "`(` after `main`")?;
        self.expect(&TokenKind::RightParen, "`)` after `main`")?;
        let body = self.parse_block()?;
        Ok(Function {
            line: start.line,
            column: start.column,
            name,
            generic_parameters: Vec::new(),
            annotations: Vec::new(),
            return_type: Some(TypeName {
                line: start.line,
                column: start.column,
                name: "int".to_owned(),
                generic_arguments: Vec::new(),
            }),
            parameters: Vec::new(),
            body,
            is_entry: true,
        })
    }

    fn parse_parameters(&mut self) -> Result<Vec<Parameter>, ParseError> {
        self.expect(&TokenKind::LeftParen, "`(` before function parameters")?;
        let mut parameters = Vec::new();

        if !self.check(&TokenKind::RightParen) {
            loop {
                let variadic = self.matches(&TokenKind::Star);
                let name_token = self.expect_identifier_token("a parameter name")?;
                let name = match name_token.kind {
                    TokenKind::Identifier(name) => name,
                    _ => unreachable!("expect_identifier_token checks the token kind"),
                };
                self.expect(&TokenKind::Assign, "`=` between parameter name and type")?;
                let ty = self.parse_type()?;
                let default_value = if self.matches(&TokenKind::Arrow) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                parameters.push(Parameter {
                    line: name_token.line,
                    column: name_token.column,
                    name,
                    ty,
                    default_value,
                    variadic,
                });
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }

        self.expect(&TokenKind::RightParen, "`)` after function parameters")?;
        Ok(parameters)
    }

    fn parse_type(&mut self) -> Result<TypeName, ParseError> {
        let token = self
            .advance()
            .ok_or_else(|| self.error("expected a type name"))?;
        let name = match token.kind {
            TokenKind::Identifier(name) => name,
            TokenKind::Int => "int".to_owned(),
            TokenKind::Auto => "auto".to_owned(),
            TokenKind::Std => "std".to_owned(),
            TokenKind::Linux => "linux".to_owned(),
            _ => {
                return Err(self.error_at(
                    &token,
                    format!("expected a type name, found `{}`", token.lexeme),
                ))
            }
        };
        let generic_arguments = if self.matches(&TokenKind::Lt) {
            let mut arguments = Vec::new();
            loop {
                arguments.push(self.parse_type()?);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Gt, "`>` after generic type arguments")?;
            arguments
        } else {
            Vec::new()
        };
        Ok(TypeName {
            line: token.line,
            column: token.column,
            name,
            generic_arguments,
        })
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let start = self.expect(&TokenKind::LeftBrace, "`{` to begin a code block")?;
        let mut statements = Vec::new();

        while !self.is_at_end() && !self.check(&TokenKind::RightBrace) {
            statements.push(self.parse_statement()?);
        }

        self.expect(&TokenKind::RightBrace, "`}` to close the code block")?;
        Ok(Block {
            line: start.line,
            column: start.column,
            statements,
        })
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::Semicolon) => {
                let token = self.advance().expect("peeked semicolon");
                Ok(Statement::Empty(EmptyStatement {
                    line: token.line,
                    column: token.column,
                }))
            }
            Some(TokenKind::If) => self.parse_if(),
            Some(TokenKind::While) => self.parse_while(),
            Some(TokenKind::For) => self.parse_for(),
            Some(TokenKind::Try) => self.parse_try(),
            Some(TokenKind::Const) => self.parse_const(),
            Some(TokenKind::Return) => self.parse_return(),
            Some(TokenKind::Stop) => Err(self.error(
                "`stop;` marks the end of the source and is only allowed after all declarations",
            )),
            Some(TokenKind::RulesBang) => self.parse_rules(),
            Some(TokenKind::GcBang) => self.parse_gc_control(),
            Some(TokenKind::Thread) => self.parse_thread(),
            Some(TokenKind::Spawn) => self.parse_spawn(),
            Some(TokenKind::Join) => self.parse_join(),
            Some(TokenKind::Break) => self.parse_break(),
            Some(TokenKind::Continue) => self.parse_continue(),
            Some(TokenKind::Identifier(_) | TokenKind::Fn) if self.is_assignment_ahead() => {
                self.parse_assignment()
            }
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_assignment(&mut self) -> Result<Statement, ParseError> {
        let name_token = self.advance().expect("peeked identifier");
        let name = match name_token.kind {
            TokenKind::Identifier(name) => name.clone(),
            TokenKind::Fn => "fn".to_owned(),
            _ => unreachable!("assignment starts with an identifier"),
        };
        let mut target_expression = Expr {
            line: name_token.line,
            column: name_token.column,
            kind: ExprKind::Identifier(name.clone()),
        };
        loop {
            if self.matches(&TokenKind::LeftBracket) {
                let bracket = self.tokens[self.current - 1].clone();
                let index = self.parse_subscript(&bracket)?;
                target_expression = Expr {
                    line: bracket.line,
                    column: bracket.column,
                    kind: ExprKind::Index {
                        container: Box::new(target_expression),
                        index: Box::new(index),
                    },
                };
            } else if self.matches(&TokenKind::Dot) {
                let dot = self.tokens[self.current - 1].clone();
                let property = self.expect_identifier("a member name")?;
                target_expression = Expr {
                    line: dot.line,
                    column: dot.column,
                    kind: ExprKind::Member {
                        object: Box::new(target_expression),
                        name: property,
                    },
                };
            } else {
                break;
            }
        }
        let target = match target_expression.kind {
            ExprKind::Identifier(identifier) => AssignmentTarget {
                line: target_expression.line,
                column: target_expression.column,
                kind: AssignmentTargetKind::Variable(identifier),
            },
            ExprKind::Index { container, index } => AssignmentTarget {
                line: target_expression.line,
                column: target_expression.column,
                kind: AssignmentTargetKind::Index { container, index },
            },
            ExprKind::Member { object, name } => AssignmentTarget {
                line: target_expression.line,
                column: target_expression.column,
                kind: AssignmentTargetKind::Member { object, name },
            },
            _ => unreachable!("assignment target is a variable or indexed variable"),
        };
        self.expect(&TokenKind::Assign, "`=` after the variable name")?;
        let ty = self.parse_type()?;
        self.expect(&TokenKind::Arrow, "`->` between assignment type and value")?;
        let value = self.parse_expression()?;
        self.expect(&TokenKind::Semicolon, "`;` after the assignment")?;
        Ok(Statement::Assignment(AssignmentStatement {
            line: name_token.line,
            column: name_token.column,
            name,
            target,
            ty,
            value,
        }))
    }

    fn parse_expression_statement(&mut self) -> Result<Statement, ParseError> {
        let expression = self.parse_expression()?;
        if !matches!(
            expression.kind,
            ExprKind::Call { .. }
                | ExprKind::MethodCall { .. }
                | ExprKind::Help { .. }
                | ExprKind::Docstring { .. }
        ) {
            return Err(ParseError {
                message: "a bare expression is not a statement; use an assignment or function call"
                    .to_owned(),
                line: expression.line,
                column: expression.column,
            });
        }
        let line = expression.line;
        let column = expression.column;
        self.expect(&TokenKind::Semicolon, "`;` after the function call")?;
        Ok(Statement::Expression(ExpressionStatement {
            line,
            column,
            expression,
        }))
    }

    fn parse_if(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::If, "`if`")?;
        let mut branches = Vec::new();
        branches.push(self.parse_conditional_branch("`(` after `if`")?);

        while self.matches(&TokenKind::Elif) {
            branches.push(self.parse_conditional_branch("`(` after `elif`")?);
        }

        let else_block = if self.matches(&TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };
        self.matches(&TokenKind::Semicolon);

        Ok(Statement::If(IfStatement {
            line: start.line,
            column: start.column,
            branches,
            else_block,
        }))
    }

    fn parse_conditional_branch(
        &mut self,
        left_paren_description: &str,
    ) -> Result<(Expr, Block), ParseError> {
        self.expect(&TokenKind::LeftParen, left_paren_description)?;
        let condition = self.parse_expression()?;
        self.expect(&TokenKind::RightParen, "`)` after the condition")?;
        let block = self.parse_block()?;
        Ok((condition, block))
    }

    fn parse_while(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::While, "`while`")?;
        self.expect(&TokenKind::LeftParen, "`(` after `while`")?;
        let condition = self.parse_expression()?;
        self.expect(&TokenKind::RightParen, "`)` after the `while` condition")?;
        self.loop_depth += 1;
        let body_result = self.parse_block();
        self.loop_depth -= 1;
        let body = body_result?;
        self.matches(&TokenKind::Semicolon);
        Ok(Statement::While(WhileStatement {
            line: start.line,
            column: start.column,
            condition,
            body,
        }))
    }

    fn parse_for(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::For, "`for`")?;
        let variable = self.expect_identifier_token("a loop variable")?;
        let variable_name = match variable.kind {
            TokenKind::Identifier(name) => name,
            _ => unreachable!("identifier token was checked"),
        };
        self.expect(&TokenKind::In, "`in` after the loop variable")?;
        let iterable = self.parse_expression()?;
        self.loop_depth += 1;
        let body_result = self.parse_block();
        self.loop_depth -= 1;
        let body = body_result?;
        self.matches(&TokenKind::Semicolon);
        Ok(Statement::For(ForStatement {
            line: start.line,
            column: start.column,
            variable: variable_name,
            variable_line: variable.line,
            variable_column: variable.column,
            iterable,
            body,
        }))
    }

    fn parse_try(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::Try, "`try`")?;
        let body = self.parse_block()?;
        let mut handlers = Vec::new();
        while let Some(TokenKind::Except) = self.peek_kind() {
            let handler_start = self.advance().expect("peeked except");
            let error_type = if matches!(self.peek_kind(), Some(TokenKind::Identifier(_))) {
                let exception = self.expect_identifier_token("an exception type")?;
                match exception.kind {
                    TokenKind::Identifier(name) => Some(name),
                    _ => unreachable!("identifier token was checked"),
                }
            } else {
                None
            };
            let handler_body = self.parse_block()?;
            handlers.push(ExceptHandler {
                line: handler_start.line,
                column: handler_start.column,
                error_type,
                body: handler_body,
            });
        }
        let else_block = if self.matches(&TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };
        let finally_block = if self.matches(&TokenKind::Finally) {
            Some(self.parse_block()?)
        } else {
            None
        };
        if handlers.is_empty() && finally_block.is_none() {
            return Err(self.error_at(
                &start,
                "a `try` statement requires at least one `except` or a `finally` block",
            ));
        }
        self.matches(&TokenKind::Semicolon);
        Ok(Statement::Try(TryStatement {
            line: start.line,
            column: start.column,
            body,
            handlers,
            else_block,
            finally_block,
        }))
    }

    fn parse_const(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::Const, "`const`")?;
        let Statement::Assignment(assignment) = self.parse_assignment()? else {
            unreachable!("const declaration must be an assignment")
        };
        Ok(Statement::Const(ConstStatement {
            line: start.line,
            column: start.column,
            assignment,
        }))
    }

    fn parse_class(&mut self) -> Result<ClassDefinition, ParseError> {
        let start = self.expect(&TokenKind::Class, "`class`")?;
        let name = self.expect_identifier("a class name")?;
        let generic_parameters = self.parse_generic_parameters()?;
        let mut bases = Vec::new();
        if self.matches(&TokenKind::LeftParen) {
            if !self.check(&TokenKind::RightParen) {
                loop {
                    bases.push(self.expect_identifier("a base class name")?);
                    if !self.matches(&TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(&TokenKind::RightParen, "`)` after base classes")?;
        }
        self.expect(&TokenKind::LeftBrace, "`{` to begin the class body")?;
        let mut members = Vec::new();
        while !self.is_at_end() && !self.check(&TokenKind::RightBrace) {
            if self.matches(&TokenKind::Semicolon) {
                continue;
            }
            if self.check(&TokenKind::Def) {
                members.push(ClassMember::Function(self.parse_function()?));
            } else if matches!(self.peek_kind(), Some(TokenKind::Identifier(_)))
                && self.is_assignment_ahead()
            {
                let Statement::Assignment(assignment) = self.parse_assignment()? else {
                    unreachable!("class field parses as assignment")
                };
                members.push(ClassMember::Assignment(assignment));
            } else {
                return Err(self.error("expected a method or field assignment in class body"));
            }
        }
        self.expect(&TokenKind::RightBrace, "`}` to close the class body")?;
        self.matches(&TokenKind::Semicolon);
        Ok(ClassDefinition {
            line: start.line,
            column: start.column,
            name,
            generic_parameters,
            bases,
            members,
        })
    }

    fn parse_return(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::Return, "`return`")?;
        self.expect(&TokenKind::Code, "`code` after `return`")?;
        self.expect(&TokenKind::Arrow, "`->` after `return code`")?;
        let value = self.parse_expression()?;
        Ok(Statement::Return(ReturnStatement {
            line: start.line,
            column: start.column,
            value,
        }))
    }

    fn parse_break(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::Break, "`break`")?;
        if self.loop_depth == 0 {
            return Err(self.error_at(&start, "`break` is only valid inside a loop"));
        }
        self.expect(&TokenKind::Semicolon, "`;` after `break`")?;
        Ok(Statement::Break(BreakStatement {
            line: start.line,
            column: start.column,
        }))
    }

    fn parse_continue(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::Continue, "`continue`")?;
        if self.loop_depth == 0 {
            return Err(self.error_at(&start, "`continue` is only valid inside a loop"));
        }
        self.expect(&TokenKind::Semicolon, "`;` after `continue`")?;
        Ok(Statement::Continue(ContinueStatement {
            line: start.line,
            column: start.column,
        }))
    }

    fn parse_rules(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::RulesBang, "`rules!`")?;
        let operation = self.expect_identifier("`write` or `output` after `rules!`")?;
        if operation != "write" && operation != "output" {
            return Err(self.error_at(
                &self.tokens[self.current - 1],
                "expected `write` or `output` after `rules!`",
            ));
        }
        self.expect(&TokenKind::Arrow, "`->` after the rules operation")?;
        let value = self.parse_expression()?;
        self.expect(&TokenKind::Semicolon, "`;` after the rules command")?;

        Ok(Statement::Rules(RulesStatement {
            line: start.line,
            column: start.column,
            operation: Some(operation),
            value: Some(value),
            body: None,
        }))
    }

    fn parse_gc_control(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::GcBang, "`gc!`")?;
        let operation = self
            .advance()
            .ok_or_else(|| self.error("expected `collect`, `pause`, or `resume` after `gc!`"))?;
        let TokenKind::Identifier(operation_name) = &operation.kind else {
            return Err(self.error_at(
                &operation,
                "expected `collect`, `pause`, or `resume` after `gc!`",
            ));
        };
        let operation_kind = match operation_name.as_str() {
            "collect" => GcOperation::Collect,
            "pause" => GcOperation::Pause,
            "resume" => GcOperation::Resume,
            _ => {
                return Err(self.error_at(
                    &operation,
                    "unknown GC operation; expected `collect`, `pause`, or `resume`",
                ))
            }
        };
        self.expect(&TokenKind::Semicolon, "`;` after the GC operation")?;
        Ok(Statement::GcControl(GcControlStatement {
            line: start.line,
            column: start.column,
            operation: operation_kind,
        }))
    }

    fn parse_thread(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::Thread, "`thread`")?;
        let name = self.expect_identifier("a thread name")?;
        let body = self.parse_block()?;
        self.matches(&TokenKind::Semicolon);
        Ok(Statement::Thread(ThreadStatement {
            line: start.line,
            column: start.column,
            name,
            body,
        }))
    }

    fn parse_spawn(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::Spawn, "`spawn`")?;
        let name = self.expect_identifier("a thread name after `spawn`")?;
        self.expect(&TokenKind::Semicolon, "`;` after `spawn`")?;
        Ok(Statement::Spawn(SpawnStatement {
            line: start.line,
            column: start.column,
            name,
        }))
    }

    fn parse_join(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect(&TokenKind::Join, "`join`")?;
        let name = self.expect_identifier("a thread name after `join`")?;
        self.expect(&TokenKind::Semicolon, "`;` after `join`")?;
        Ok(Statement::Join(JoinStatement {
            line: start.line,
            column: start.column,
            name,
        }))
    }

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut expression = self.parse_and()?;
        while self.matches(&TokenKind::Or) {
            let token = self.tokens[self.current - 1].clone();
            let right = self.parse_and()?;
            expression = Self::binary_expression(token, BinaryOperator::Or, expression, right);
        }
        Ok(expression)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut expression = self.parse_comparison()?;
        while self.matches(&TokenKind::And) {
            let token = self.tokens[self.current - 1].clone();
            let right = self.parse_comparison()?;
            expression = Self::binary_expression(token, BinaryOperator::And, expression, right);
        }
        Ok(expression)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expression = self.parse_addition()?;
        loop {
            let operator = match self.peek_kind() {
                Some(TokenKind::Ge) => BinaryOperator::GreaterEqual,
                Some(TokenKind::Gt) => BinaryOperator::Greater,
                Some(TokenKind::Le) => BinaryOperator::LessEqual,
                Some(TokenKind::Lt) => BinaryOperator::Less,
                Some(TokenKind::EqEq) => BinaryOperator::Equal,
                Some(TokenKind::BangEq) => BinaryOperator::NotEqual,
                _ => break,
            };
            let token = self.advance().expect("peeked comparison operator");
            let right = self.parse_addition()?;
            expression = Self::binary_expression(token, operator, expression, right);
        }
        Ok(expression)
    }

    fn parse_addition(&mut self) -> Result<Expr, ParseError> {
        let mut expression = self.parse_multiplication()?;
        loop {
            let operator = match self.peek_kind() {
                Some(TokenKind::Plus) => BinaryOperator::Add,
                Some(TokenKind::Minus) => BinaryOperator::Subtract,
                _ => break,
            };
            let token = self.advance().expect("peeked addition operator");
            let right = self.parse_multiplication()?;
            expression = Self::binary_expression(token, operator, expression, right);
        }
        Ok(expression)
    }

    fn parse_multiplication(&mut self) -> Result<Expr, ParseError> {
        let mut expression = self.parse_unary()?;
        loop {
            let operator = match self.peek_kind() {
                Some(TokenKind::Star) => BinaryOperator::Multiply,
                Some(TokenKind::Slash) => BinaryOperator::Divide,
                Some(TokenKind::Percent) => BinaryOperator::Modulo,
                _ => break,
            };
            let token = self.advance().expect("peeked multiplication operator");
            let right = self.parse_unary()?;
            expression = Self::binary_expression(token, operator, expression, right);
        }
        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.matches(&TokenKind::Minus) {
            let token = self.tokens[self.current - 1].clone();
            let operand = self.parse_unary()?;
            return Ok(Expr {
                line: token.line,
                column: token.column,
                kind: ExprKind::Unary {
                    operator: UnaryOperator::Negate,
                    operand: Box::new(operand),
                },
            });
        }
        self.parse_not()
    }

    fn parse_not(&mut self) -> Result<Expr, ParseError> {
        if self.matches(&TokenKind::Not) {
            let token = self.tokens[self.current - 1].clone();
            let operand = self.parse_unary()?;
            return Ok(Expr {
                line: token.line,
                column: token.column,
                kind: ExprKind::Unary {
                    operator: UnaryOperator::Not,
                    operand: Box::new(operand),
                },
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self
            .advance()
            .ok_or_else(|| self.error("expected an expression"))?;
        let token_kind = match token.kind.clone() {
            TokenKind::Fn => TokenKind::Identifier("fn".to_owned()),
            kind => kind,
        };
        let kind = match token_kind {
            TokenKind::Number(number) => ExprKind::Number(number),
            TokenKind::StringLiteral(value) => ExprKind::String(value),
            TokenKind::True => ExprKind::Boolean(true),
            TokenKind::False => ExprKind::Boolean(false),
            TokenKind::Identifier(name) if self.matches(&TokenKind::LeftParen) => {
                let arguments = self.parse_call_arguments()?;
                match name.as_str() {
                    "help" => {
                        if arguments.len() != 1 {
                            return Err(
                                self.error_at(&token, "`help` expects exactly one expression")
                            );
                        }
                        ExprKind::Help {
                            argument: Box::new(arguments.into_iter().next().unwrap()),
                        }
                    }
                    "docstring" => {
                        if arguments.len() != 1 {
                            return Err(self.error_at(
                                &token,
                                "`docstring` expects exactly one string literal",
                            ));
                        }
                        let argument = arguments.into_iter().next().unwrap();
                        let ExprKind::String(value) = argument.kind else {
                            return Err(self.error_at(
                                &token,
                                "`docstring` expects exactly one string literal",
                            ));
                        };
                        ExprKind::Docstring { value }
                    }
                    _ => ExprKind::Call {
                        callee: name,
                        arguments,
                    },
                }
            }
            TokenKind::Identifier(name) => ExprKind::Identifier(name),
            TokenKind::Lambda => {
                let parameters = self.parse_parameters()?;
                self.expect(&TokenKind::Arrow, "`->` before the lambda body")?;
                let body = self.parse_expression()?;
                ExprKind::Lambda {
                    parameters,
                    body: Box::new(body),
                }
            }
            TokenKind::LeftBracket => self.parse_list_literal()?,
            TokenKind::LeftBrace => self.parse_dict_literal()?,
            TokenKind::LeftParen => {
                if self.matches(&TokenKind::RightParen) {
                    ExprKind::TupleLiteral(Vec::new())
                } else {
                    let first = self.parse_expression()?;
                    if self.matches(&TokenKind::Comma) {
                        let mut elements = vec![first];
                        while !self.check(&TokenKind::RightParen) {
                            elements.push(self.parse_expression()?);
                            if !self.matches(&TokenKind::Comma) {
                                break;
                            }
                        }
                        self.expect(&TokenKind::RightParen, "`)` after tuple elements")?;
                        ExprKind::TupleLiteral(elements)
                    } else {
                        self.expect(&TokenKind::RightParen, "`)` after the grouped expression")?;
                        ExprKind::Grouping(Box::new(first))
                    }
                }
            }
            _ => {
                return Err(self.error_at(
                    &token,
                    format!("expected an expression, found `{}`", token.lexeme),
                ))
            }
        };
        let expression = Expr {
            line: token.line,
            column: token.column,
            kind,
        };
        self.parse_postfix(expression)
    }

    fn parse_list_literal(&mut self) -> Result<ExprKind, ParseError> {
        let mut elements = Vec::new();
        if self.check(&TokenKind::RightBracket) {
            self.advance();
            return Ok(ExprKind::ListLiteral(elements));
        }

        elements.push(self.parse_expression()?);
        if self.matches(&TokenKind::For) {
            let (variable, _, _) = self.parse_comprehension_variable()?;
            self.expect(&TokenKind::In, "`in` in list comprehension")?;
            let iterable = self.parse_expression()?;
            let condition = if self.matches(&TokenKind::If) {
                Some(Box::new(self.parse_expression()?))
            } else {
                None
            };
            self.expect(&TokenKind::RightBracket, "`]` after list comprehension")?;
            return Ok(ExprKind::ListComprehension {
                element: Box::new(elements.remove(0)),
                variable,
                iterable: Box::new(iterable),
                condition,
            });
        }

        while self.matches(&TokenKind::Comma) {
            if self.check(&TokenKind::RightBracket) {
                break;
            }
            elements.push(self.parse_expression()?);
        }
        self.expect(&TokenKind::RightBracket, "`]` after list elements")?;
        Ok(ExprKind::ListLiteral(elements))
    }

    fn parse_dict_literal(&mut self) -> Result<ExprKind, ParseError> {
        let mut entries = Vec::new();
        if self.check(&TokenKind::RightBrace) {
            self.advance();
            return Ok(ExprKind::DictLiteral(entries));
        }

        let key = self.parse_expression()?;
        self.expect(&TokenKind::Colon, "`:` between dictionary key and value")?;
        let value = self.parse_expression()?;
        if self.matches(&TokenKind::For) {
            let (variable, _, _) = self.parse_comprehension_variable()?;
            self.expect(&TokenKind::In, "`in` in dictionary comprehension")?;
            let iterable = self.parse_expression()?;
            let condition = if self.matches(&TokenKind::If) {
                Some(Box::new(self.parse_expression()?))
            } else {
                None
            };
            self.expect(&TokenKind::RightBrace, "`}` after dictionary comprehension")?;
            return Ok(ExprKind::DictComprehension {
                key: Box::new(key),
                value: Box::new(value),
                variable,
                iterable: Box::new(iterable),
                condition,
            });
        }

        entries.push((key, value));
        while self.matches(&TokenKind::Comma) {
            if self.check(&TokenKind::RightBrace) {
                break;
            }
            let key = self.parse_expression()?;
            self.expect(&TokenKind::Colon, "`:` between dictionary key and value")?;
            let value = self.parse_expression()?;
            entries.push((key, value));
        }
        self.expect(&TokenKind::RightBrace, "`}` after dictionary entries")?;
        Ok(ExprKind::DictLiteral(entries))
    }

    fn parse_comprehension_variable(&mut self) -> Result<(String, usize, usize), ParseError> {
        let token = self.expect_identifier_token("a comprehension variable")?;
        let name = match token.kind {
            TokenKind::Identifier(name) => name,
            _ => unreachable!("identifier token was checked"),
        };
        Ok((name, token.line, token.column))
    }

    fn parse_postfix(&mut self, mut expression: Expr) -> Result<Expr, ParseError> {
        loop {
            if self.matches(&TokenKind::LeftBracket) {
                let bracket = self.tokens[self.current - 1].clone();
                let index = self.parse_subscript(&bracket)?;
                expression = Expr {
                    line: bracket.line,
                    column: bracket.column,
                    kind: ExprKind::Index {
                        container: Box::new(expression),
                        index: Box::new(index),
                    },
                };
            } else if self.matches(&TokenKind::Dot) {
                let dot = self.tokens[self.current - 1].clone();
                let method = self.expect_identifier("a container method name")?;
                if self.matches(&TokenKind::LeftParen) {
                    let arguments = self.parse_call_arguments()?;
                    expression = Expr {
                        line: dot.line,
                        column: dot.column,
                        kind: ExprKind::MethodCall {
                            receiver: Box::new(expression),
                            method,
                            arguments,
                        },
                    };
                } else {
                    expression = Expr {
                        line: dot.line,
                        column: dot.column,
                        kind: ExprKind::Member {
                            object: Box::new(expression),
                            name: method,
                        },
                    };
                }
            } else {
                break;
            }
        }
        Ok(expression)
    }

    fn parse_subscript(&mut self, bracket: &Token) -> Result<Expr, ParseError> {
        let start = if self.check(&TokenKind::Colon) || self.check(&TokenKind::RightBracket) {
            None
        } else {
            Some(Box::new(self.parse_expression()?))
        };

        if self.matches(&TokenKind::Colon) {
            let end = if self.check(&TokenKind::Colon) || self.check(&TokenKind::RightBracket) {
                None
            } else {
                Some(Box::new(self.parse_expression()?))
            };
            let step = if self.matches(&TokenKind::Colon) {
                if self.check(&TokenKind::RightBracket) {
                    None
                } else {
                    Some(Box::new(self.parse_expression()?))
                }
            } else {
                None
            };
            self.expect(&TokenKind::RightBracket, "`]` after slice")?;
            return Ok(Expr {
                line: bracket.line,
                column: bracket.column,
                kind: ExprKind::Slice { start, end, step },
            });
        }

        let index = start.ok_or_else(|| self.error("expected an index or slice"))?;
        self.expect(&TokenKind::RightBracket, "`]` after the index")?;
        Ok(*index)
    }

    fn parse_call_arguments(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut arguments = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                arguments.push(self.parse_expression()?);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RightParen, "`)` after call arguments")?;
        Ok(arguments)
    }

    fn binary_expression(token: Token, operator: BinaryOperator, left: Expr, right: Expr) -> Expr {
        Expr {
            line: token.line,
            column: token.column,
            kind: ExprKind::Binary {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            },
        }
    }

    fn expect_identifier(&mut self, description: &str) -> Result<String, ParseError> {
        let token = self.expect_identifier_token(description)?;
        match token.kind {
            TokenKind::Identifier(name) => Ok(name),
            TokenKind::Fn => Ok("fn".to_owned()),
            _ => unreachable!("expect_identifier_token checks the token kind"),
        }
    }

    fn expect_identifier_token(&mut self, description: &str) -> Result<Token, ParseError> {
        if matches!(
            self.peek_kind(),
            Some(TokenKind::Identifier(_) | TokenKind::Fn)
        ) {
            Ok(self.advance().expect("peeked identifier"))
        } else {
            Err(self.error(format!("expected {description}")))
        }
    }

    fn expect(&mut self, kind: &TokenKind, description: &str) -> Result<Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance().expect("matched token"))
        } else {
            Err(self.error(format!("expected {description}")))
        }
    }

    fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.current += 1;
            true
        } else {
            false
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek_kind()
            .is_some_and(|actual| std::mem::discriminant(actual) == std::mem::discriminant(kind))
    }

    fn advance(&mut self) -> Option<Token> {
        if self.is_at_end() {
            return None;
        }
        let token = self.tokens[self.current].clone();
        self.current += 1;
        Some(token)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.current).map(|token| &token.kind)
    }

    fn is_assignment_ahead(&self) -> bool {
        let mut index = self.current + 1;
        loop {
            match self.tokens.get(index).map(|token| &token.kind) {
                Some(TokenKind::Assign) => return true,
                Some(TokenKind::Dot) => {
                    if !matches!(
                        self.tokens.get(index + 1).map(|token| &token.kind),
                        Some(TokenKind::Identifier(_))
                    ) {
                        return false;
                    }
                    index += 2;
                }
                Some(TokenKind::LeftBracket) => {
                    let mut depth = 1usize;
                    index += 1;
                    while depth > 0 {
                        match self.tokens.get(index).map(|token| &token.kind) {
                            Some(TokenKind::LeftBracket) => depth += 1,
                            Some(TokenKind::RightBracket) => depth -= 1,
                            _ => {}
                        }
                        if index >= self.tokens.len() {
                            return false;
                        }
                        index += 1;
                    }
                }
                _ => return false,
            }
        }
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len()
    }

    fn position(&self) -> (usize, usize) {
        self.tokens
            .get(self.current)
            .map_or((self.eof_line, self.eof_column), |token| {
                (token.line, token.column)
            })
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        let (line, column) = self.position();
        ParseError {
            message: message.into(),
            line,
            column,
        }
    }

    fn error_at(&self, token: &Token, message: impl Into<String>) -> ParseError {
        ParseError {
            message: message.into(),
            line: token.line,
            column: token.column,
        }
    }
}

fn source_end_position(source: &str) -> (usize, usize) {
    source.chars().fold((1, 1), |(line, column), character| {
        if character == '\n' {
            (line + 1, 1)
        } else {
            (line, column + 1)
        }
    })
}
