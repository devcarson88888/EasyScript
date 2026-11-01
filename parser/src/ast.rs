#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub line: usize,
    pub column: usize,
    pub items: Vec<TopLevelItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopLevelItem {
    Header(HeaderDeclaration),
    Import(ImportDeclaration),
    EspkgPackage(EspkgPackageDeclaration),
    Function(Function),
    Struct(StructDefinition),
    Enum(EnumDefinition),
    ExternFunction(ExternFunction),
    Statement(Statement),
    Class(ClassDefinition),
    TypeAlias(TypeAlias),
    Stop(StopStatement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderDeclaration {
    pub line: usize,
    pub column: usize,
    pub system: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDeclaration {
    pub line: usize,
    pub column: usize,
    pub library: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspkgPackageDeclaration {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub generic_parameters: Vec<GenericParameter>,
    pub annotations: Vec<Annotation>,
    pub return_type: Option<TypeName>,
    pub parameters: Vec<Parameter>,
    pub body: Block,
    pub is_entry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParameter {
    pub line: usize,
    pub column: usize,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub line: usize,
    pub column: usize,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDefinition {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub generic_parameters: Vec<GenericParameter>,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub ty: TypeName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDefinition {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub generic_parameters: Vec<GenericParameter>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub payload: EnumVariantPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumVariantPayload {
    Unit,
    Tuple(Vec<TypeName>),
    Struct(Vec<StructField>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternFunction {
    pub line: usize,
    pub column: usize,
    pub abi: String,
    pub name: String,
    pub generic_parameters: Vec<GenericParameter>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub line: usize,
    pub column: usize,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub ty: TypeName,
    pub default_value: Option<Expr>,
    pub variadic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDefinition {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub generic_parameters: Vec<GenericParameter>,
    pub bases: Vec<String>,
    pub members: Vec<ClassMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassMember {
    Function(Function),
    Assignment(AssignmentStatement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAlias {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub target: TypeName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeName {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub generic_arguments: Vec<TypeName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Assignment(AssignmentStatement),
    Expression(ExpressionStatement),
    If(IfStatement),
    While(WhileStatement),
    Return(ReturnStatement),
    Stop(StopStatement),
    Rules(RulesStatement),
    Break(BreakStatement),
    Continue(ContinueStatement),
    For(ForStatement),
    Try(TryStatement),
    Const(ConstStatement),
    GcControl(GcControlStatement),
    Thread(ThreadStatement),
    Spawn(SpawnStatement),
    Join(JoinStatement),
    Empty(EmptyStatement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentStatement {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub target: AssignmentTarget,
    pub ty: TypeName,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentTarget {
    pub line: usize,
    pub column: usize,
    pub kind: AssignmentTargetKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentTargetKind {
    Variable(String),
    Member {
        object: Box<Expr>,
        name: String,
    },
    Index {
        container: Box<Expr>,
        index: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionStatement {
    pub line: usize,
    pub column: usize,
    pub expression: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStatement {
    pub line: usize,
    pub column: usize,
    pub branches: Vec<(Expr, Block)>,
    pub else_block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileStatement {
    pub line: usize,
    pub column: usize,
    pub condition: Expr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnStatement {
    pub line: usize,
    pub column: usize,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopStatement {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesStatement {
    pub line: usize,
    pub column: usize,
    pub operation: Option<String>,
    pub value: Option<Expr>,
    pub body: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakStatement {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinueStatement {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForStatement {
    pub line: usize,
    pub column: usize,
    pub variable: String,
    pub variable_line: usize,
    pub variable_column: usize,
    pub iterable: Expr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryStatement {
    pub line: usize,
    pub column: usize,
    pub body: Block,
    pub handlers: Vec<ExceptHandler>,
    pub else_block: Option<Block>,
    pub finally_block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptHandler {
    pub line: usize,
    pub column: usize,
    pub error_type: Option<String>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstStatement {
    pub line: usize,
    pub column: usize,
    pub assignment: AssignmentStatement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyStatement {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcControlStatement {
    pub line: usize,
    pub column: usize,
    pub operation: GcOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcOperation {
    Collect,
    Pause,
    Resume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadStatement {
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnStatement {
    pub line: usize,
    pub column: usize,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinStatement {
    pub line: usize,
    pub column: usize,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    pub line: usize,
    pub column: usize,
    pub kind: ExprKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    Number(String),
    String(String),
    Boolean(bool),
    Identifier(String),
    Call {
        callee: String,
        arguments: Vec<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        arguments: Vec<Expr>,
    },
    Member {
        object: Box<Expr>,
        name: String,
    },
    DictLiteral(Vec<(Expr, Expr)>),
    ListLiteral(Vec<Expr>),
    TupleLiteral(Vec<Expr>),
    Index {
        container: Box<Expr>,
        index: Box<Expr>,
    },
    Slice {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        step: Option<Box<Expr>>,
    },
    ListComprehension {
        element: Box<Expr>,
        variable: String,
        iterable: Box<Expr>,
        condition: Option<Box<Expr>>,
    },
    DictComprehension {
        key: Box<Expr>,
        value: Box<Expr>,
        variable: String,
        iterable: Box<Expr>,
        condition: Option<Box<Expr>>,
    },
    Lambda {
        parameters: Vec<Parameter>,
        body: Box<Expr>,
    },
    Help {
        argument: Box<Expr>,
    },
    Docstring {
        value: String,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expr>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Grouping(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    GreaterEqual,
    Greater,
    LessEqual,
    Less,
    Equal,
    NotEqual,
    And,
    Or,
}
