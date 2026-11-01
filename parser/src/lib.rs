pub mod ast;
pub mod parser;
pub mod type_checker;

pub use ast::{
    Annotation, Block, ClassDefinition, ClassMember, EnumDefinition, EnumVariant,
    EnumVariantPayload, EspkgPackageDeclaration, Expr, ExprKind, ExternFunction,
    GcControlStatement, GcOperation, GenericParameter, JoinStatement, Program, SpawnStatement,
    Statement, StructDefinition, StructField, ThreadStatement, TopLevelItem, TypeName,
};
pub use easyscript_lexer as lexer;
pub use easyscript_lexer::{lex, LexError, Token, TokenKind};
pub use parser::{parse, parse_tokens, ParseError};
pub use type_checker::{validate_mutability, TypeError};
