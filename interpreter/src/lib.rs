use easyscript_parser::ast::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::rc::Rc;

type Env = Rc<RefCell<Scope>>;

#[derive(Clone)]
pub struct Scope {
    values: HashMap<String, Value>,
    parent: Option<Env>,
}

impl Scope {
    fn new(parent: Option<Env>) -> Env {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            parent,
        }))
    }

    fn define(env: &Env, name: impl Into<String>, value: Value) {
        env.borrow_mut().values.insert(name.into(), value);
    }

    fn get(env: &Env, name: &str) -> Option<Value> {
        let (value, parent) = {
            let scope = env.borrow();
            (scope.values.get(name).cloned(), scope.parent.clone())
        };
        value.or_else(|| parent.and_then(|parent| Self::get(&parent, name)))
    }

    fn assign(env: &Env, name: &str, value: Value) {
        let parent = {
            let mut scope = env.borrow_mut();
            if scope.values.contains_key(name) {
                scope.values.insert(name.to_owned(), value);
                return;
            }
            scope.parent.clone()
        };
        if let Some(parent) = parent {
            if Self::contains(&parent, name) {
                Self::assign(&parent, name, value);
                return;
            }
        }
        env.borrow_mut().values.insert(name.to_owned(), value);
    }

    fn contains(env: &Env, name: &str) -> bool {
        let (contains, parent) = {
            let scope = env.borrow();
            (scope.values.contains_key(name), scope.parent.clone())
        };
        contains || parent.is_some_and(|parent| Self::contains(&parent, name))
    }
}

#[derive(Clone)]
pub enum Value {
    Null,
    StdLibrary,
    Number(i64),
    String(String),
    Boolean(bool),
    List(Rc<RefCell<Vec<Value>>>),
    Tuple(Vec<Value>),
    Dict(Rc<RefCell<Vec<(Value, Value)>>>),
    Range { start: i64, stop: i64, step: i64 },
    StructType(StructDefinition),
    EnumType(EnumDefinition),
    ClassType(ClassDefinition),
    StructInstance(Rc<RefCell<StructInstance>>),
    EnumInstance(Box<EnumInstance>),
    ClassInstance(Rc<RefCell<ClassInstance>>),
    Callable(Rc<Callable>),
}

#[derive(Clone)]
pub struct StructInstance {
    name: String,
    fields: HashMap<String, Value>,
}

#[derive(Clone)]
pub struct EnumInstance {
    name: String,
    variant: String,
    payload: EnumValuePayload,
}

#[derive(Clone)]
pub enum EnumValuePayload {
    Unit,
    Tuple(Vec<Value>),
    Struct(HashMap<String, Value>),
}

#[derive(Clone)]
pub struct ClassInstance {
    name: String,
    fields: HashMap<String, Value>,
    methods: HashMap<String, Function>,
}

#[derive(Clone)]
pub enum Callable {
    UserFunction {
        function: Function,
        closure: Option<Env>,
        bound_self: Option<Value>,
    },
    Lambda {
        parameters: Vec<Parameter>,
        body: Expr,
        closure: Env,
    },
    Native(NativeFunction),
    BoundNative {
        receiver: Value,
        method: String,
    },
}

#[derive(Clone, Copy)]
pub enum NativeFunction {
    Len,
    Range,
    Int,
    Str,
    StdWrite,
    StdOutput,
    StdPrintln,
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("None"),
            Self::StdLibrary => f.write_str("<std library>"),
            Self::Number(value) => write!(f, "{value}"),
            Self::String(value) => f.write_str(value),
            Self::Boolean(value) => write!(f, "{value}"),
            Self::List(values) => write_sequence(f, "[", "]", &values.borrow()),
            Self::Tuple(values) => write_sequence(f, "(", ")", values),
            Self::Dict(entries) => {
                f.write_str("{")?;
                for (index, (key, value)) in entries.borrow().iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{key}: {value}")?;
                }
                f.write_str("}")
            }
            Self::Range { start, stop, step } => write!(f, "range({start}, {stop}, {step})"),
            Self::StructType(definition) => write!(f, "<struct {}>", definition.name),
            Self::EnumType(definition) => write!(f, "<enum {}>", definition.name),
            Self::ClassType(definition) => write!(f, "<class {}>", definition.name),
            Self::StructInstance(instance) => {
                write!(f, "{}(", instance.borrow().name)?;
                write_named_fields(f, &instance.borrow().fields)?;
                f.write_str(")")
            }
            Self::EnumInstance(instance) => write!(f, "{}.{}", instance.name, instance.variant),
            Self::ClassInstance(instance) => write!(f, "<{} instance>", instance.borrow().name),
            Self::Callable(_) => f.write_str("<function>"),
        }
    }
}

fn write_sequence(
    f: &mut fmt::Formatter<'_>,
    left: &str,
    right: &str,
    values: &[Value],
) -> fmt::Result {
    f.write_str(left)?;
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{value}")?;
    }
    if left == "(" && values.len() == 1 {
        f.write_str(",")?;
    }
    f.write_str(right)
}

fn write_named_fields(f: &mut fmt::Formatter<'_>, fields: &HashMap<String, Value>) -> fmt::Result {
    let mut fields: Vec<_> = fields.iter().collect();
    fields.sort_by_key(|(name, _)| *name);
    for (index, (name, value)) in fields.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{name}: {value}")?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.message, self.line, self.column
        )
    }
}

impl Error for RuntimeError {}

#[derive(Debug, Clone, Default)]
pub struct RunResult {
    pub output: Vec<String>,
    pub value: Option<Value>,
}

#[derive(Clone)]
enum Flow {
    Normal,
    Return(Value),
    Break,
    Continue,
    Stop,
}

#[derive(Clone)]
enum Iteration {
    Values(std::vec::IntoIter<Value>),
    Range { current: i64, stop: i64, step: i64 },
}

impl Iterator for Iteration {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Values(values) => values.next(),
            Self::Range {
                current,
                stop,
                step,
            } => {
                if (*step > 0 && *current >= *stop) || (*step < 0 && *current <= *stop) {
                    return None;
                }
                let result = *current;
                *current = current.checked_add(*step)?;
                Some(Value::Number(result))
            }
        }
    }
}

pub struct Interpreter {
    globals: Env,
    output: Vec<String>,
    std_buffer: Option<Value>,
    steps: usize,
    std_imported: bool,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        let globals = Scope::new(None);
        for (name, native) in [
            ("len", NativeFunction::Len),
            ("range", NativeFunction::Range),
            ("int", NativeFunction::Int),
            ("str", NativeFunction::Str),
        ] {
            Scope::define(
                &globals,
                name,
                Value::Callable(Rc::new(Callable::Native(native))),
            );
        }
        Self {
            globals,
            output: Vec::new(),
            std_buffer: None,
            steps: 0,
            std_imported: false,
        }
    }

    pub fn execute(&mut self, program: &Program) -> Result<RunResult, RuntimeError> {
        self.output.clear();
        self.std_buffer = None;
        self.steps = 0;
        self.std_imported = false;
        let mut entry_point = None;
        for item in &program.items {
            match item {
                TopLevelItem::Function(function) => {
                    Scope::define(
                        &self.globals,
                        function.name.clone(),
                        Value::Callable(Rc::new(Callable::UserFunction {
                            function: function.clone(),
                            closure: None,
                            bound_self: None,
                        })),
                    );
                    if function.is_entry {
                        entry_point = Some(function.clone());
                    }
                }
                TopLevelItem::Struct(definition) => Scope::define(
                    &self.globals,
                    definition.name.clone(),
                    Value::StructType(definition.clone()),
                ),
                TopLevelItem::Enum(definition) => Scope::define(
                    &self.globals,
                    definition.name.clone(),
                    Value::EnumType(definition.clone()),
                ),
                TopLevelItem::Class(definition) => Scope::define(
                    &self.globals,
                    definition.name.clone(),
                    Value::ClassType(definition.clone()),
                ),
                TopLevelItem::Import(import) if import.library == "std" => {
                    self.std_imported = true;
                    Scope::define(&self.globals, "std", Value::StdLibrary);
                    for (name, native) in [
                        ("write", NativeFunction::StdWrite),
                        ("output", NativeFunction::StdOutput),
                    ] {
                        if Scope::get(&self.globals, name).is_none() {
                            Scope::define(
                                &self.globals,
                                name,
                                Value::Callable(Rc::new(Callable::Native(native))),
                            );
                        }
                    }
                }
                TopLevelItem::Header(_) | TopLevelItem::Import(_) | TopLevelItem::Stop(_) => {}
                TopLevelItem::Statement(statement) => {
                    return Err(runtime_error(
                        statement_position(statement),
                        "top-level statements are not executable; put runtime code inside `int main()`",
                    ));
                }
                TopLevelItem::EspkgPackage(_)
                | TopLevelItem::ExternFunction(_)
                | TopLevelItem::TypeAlias(_) => {
                    return Err(runtime_error(
                        (program.line, program.column),
                        "unsupported top-level declaration",
                    ));
                }
            }
        }

        let function = entry_point.ok_or_else(|| {
            runtime_error(
                (program.line, program.column),
                "program is missing its required `int main()` entry point",
            )
        })?;
        let value = Some(self.call(
            Callable::UserFunction {
                function: function.clone(),
                closure: None,
                bound_self: None,
            },
            Vec::new(),
            (function.line, function.column),
        )?);
        Ok(RunResult {
            output: self.output.clone(),
            value,
        })
    }

    pub fn output(&self) -> &[String] {
        &self.output
    }

    fn execute_block(&mut self, block: &Block, parent: &Env) -> Result<Flow, RuntimeError> {
        let environment = Scope::new(Some(parent.clone()));
        for statement in &block.statements {
            self.tick(statement_position(statement))?;
            let flow = self.execute_statement(statement, &environment)?;
            if !matches!(flow, Flow::Normal) {
                return Ok(flow);
            }
        }
        Ok(Flow::Normal)
    }

    fn execute_statement(
        &mut self,
        statement: &Statement,
        environment: &Env,
    ) -> Result<Flow, RuntimeError> {
        self.tick(statement_position(statement))?;
        match statement {
            Statement::Assignment(assignment) => {
                let value = self.eval(&assignment.value, environment)?;
                match &assignment.target.kind {
                    AssignmentTargetKind::Variable(name) => Scope::assign(environment, name, value),
                    AssignmentTargetKind::Member { object, name } => {
                        let object = self.eval(object, environment)?;
                        self.set_member(object, name, value, position(&assignment.target))?;
                    }
                    AssignmentTargetKind::Index { container, index } => {
                        let container = self.eval(container, environment)?;
                        let index = self.eval(index, environment)?;
                        self.set_index(container, index, value, position(&assignment.target))?;
                    }
                }
                Ok(Flow::Normal)
            }
            Statement::Expression(expression) => {
                self.eval(&expression.expression, environment)?;
                Ok(Flow::Normal)
            }
            Statement::If(statement) => {
                for (condition, block) in &statement.branches {
                    if self.eval(condition, environment)?.truthy() {
                        return self.execute_block(block, environment);
                    }
                }
                if let Some(block) = &statement.else_block {
                    self.execute_block(block, environment)
                } else {
                    Ok(Flow::Normal)
                }
            }
            Statement::While(statement) => loop {
                if !self.eval(&statement.condition, environment)?.truthy() {
                    return Ok(Flow::Normal);
                }
                match self.execute_block(&statement.body, environment)? {
                    Flow::Normal | Flow::Continue => {}
                    Flow::Break => return Ok(Flow::Normal),
                    Flow::Return(value) => return Ok(Flow::Return(value)),
                    Flow::Stop => return Ok(Flow::Stop),
                }
            },
            Statement::For(statement) => {
                let iterable = self.eval(&statement.iterable, environment)?;
                let iteration = into_iteration(iterable, (statement.line, statement.column))?;
                for value in iteration {
                    let loop_environment = Scope::new(Some(environment.clone()));
                    Scope::define(&loop_environment, statement.variable.clone(), value);
                    match self.execute_block(&statement.body, &loop_environment)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        Flow::Return(value) => return Ok(Flow::Return(value)),
                        Flow::Stop => return Ok(Flow::Stop),
                    }
                }
                Ok(Flow::Normal)
            }
            Statement::Return(statement) => {
                Ok(Flow::Return(self.eval(&statement.value, environment)?))
            }
            Statement::Stop(_) => Ok(Flow::Stop),
            Statement::Rules(statement) => {
                if let (Some(operation), Some(value)) = (&statement.operation, &statement.value) {
                    if !self.std_imported {
                        return Err(runtime_error(
                            (statement.line, statement.column),
                            "`rules!` IO commands require `from library import std;`",
                        ));
                    }
                    let value = self.eval(value, environment)?;
                    match operation.as_str() {
                        "write" => {
                            Scope::assign(environment, "a", value.clone());
                            self.call_native(
                                NativeFunction::StdWrite,
                                vec![value],
                                (statement.line, statement.column),
                            )?;
                        }
                        "output" => {
                            self.call_native(
                                NativeFunction::StdOutput,
                                vec![value],
                                (statement.line, statement.column),
                            )?;
                        }
                        _ => {
                            return Err(runtime_error(
                                (statement.line, statement.column),
                                format!("unknown rules operation `{operation}`"),
                            ))
                        }
                    }
                    Ok(Flow::Normal)
                } else if let Some(body) = &statement.body {
                    self.execute_block(body, environment)
                } else {
                    Err(runtime_error(
                        (statement.line, statement.column),
                        "incomplete `rules!` command",
                    ))
                }
            }
            Statement::Break(_) => Ok(Flow::Break),
            Statement::Continue(_) => Ok(Flow::Continue),
            Statement::Try(statement) => self.execute_try(statement, environment),
            Statement::Const(statement) => self.execute_statement(
                &Statement::Assignment(statement.assignment.clone()),
                environment,
            ),
            Statement::GcControl(_) => Ok(Flow::Normal),
            Statement::Thread(_) | Statement::Spawn(_) | Statement::Join(_) => Err(runtime_error(
                statement_position(statement),
                "thread execution is not implemented by the interpreter",
            )),
            Statement::Empty(_) => Ok(Flow::Normal),
        }
    }

    fn execute_try(
        &mut self,
        statement: &TryStatement,
        environment: &Env,
    ) -> Result<Flow, RuntimeError> {
        let body_result = self.execute_block(&statement.body, environment);
        let mut caught = false;
        let mut flow = match body_result {
            Ok(flow) => flow,
            Err(error) => {
                let handler = statement.handlers.iter().find(|handler| {
                    handler.error_type.as_deref().is_none_or(|name| {
                        name == "RuntimeError"
                            || name == "Exception"
                            || (name == "ZeroDivisionError"
                                && error.message.contains("division by zero"))
                    })
                });
                if let Some(handler) = handler {
                    caught = true;
                    self.execute_block(&handler.body, environment)?
                } else {
                    if let Some(finally_block) = &statement.finally_block {
                        self.execute_block(finally_block, environment)?;
                    }
                    return Err(error);
                }
            }
        };
        if !caught && matches!(flow, Flow::Normal) {
            if let Some(else_block) = &statement.else_block {
                flow = self.execute_block(else_block, environment)?;
            }
        }
        if let Some(finally_block) = &statement.finally_block {
            let final_flow = self.execute_block(finally_block, environment)?;
            if !matches!(final_flow, Flow::Normal) {
                flow = final_flow;
            }
        }
        Ok(flow)
    }

    fn eval(&mut self, expression: &Expr, environment: &Env) -> Result<Value, RuntimeError> {
        self.tick(position(expression))?;
        match &expression.kind {
            ExprKind::Number(number) => number.parse::<i64>().map(Value::Number).map_err(|_| {
                runtime_error(
                    position(expression),
                    "integer literal is outside the supported range",
                )
            }),
            ExprKind::String(value) => Ok(Value::String(value.clone())),
            ExprKind::Boolean(value) => Ok(Value::Boolean(*value)),
            ExprKind::Identifier(name) => Scope::get(environment, name).ok_or_else(|| {
                runtime_error(position(expression), format!("undefined name `{name}`"))
            }),
            ExprKind::Call { callee, arguments } => {
                let value = Scope::get(environment, callee).ok_or_else(|| {
                    runtime_error(position(expression), format!("undefined name `{callee}`"))
                })?;
                let arguments = self.eval_arguments(arguments, environment)?;
                self.call_value(value, arguments, position(expression))
            }
            ExprKind::MethodCall {
                receiver,
                method,
                arguments,
            } => {
                let receiver = self.eval(receiver, environment)?;
                let arguments = self.eval_arguments(arguments, environment)?;
                self.call_method(receiver, method, arguments, position(expression))
            }
            ExprKind::Member { object, name } => {
                let object = self.eval(object, environment)?;
                self.get_member(object, name, position(expression))
            }
            ExprKind::DictLiteral(entries) => {
                let mut values = Vec::new();
                for (key, value) in entries {
                    values.push((self.eval(key, environment)?, self.eval(value, environment)?));
                }
                Ok(Value::Dict(Rc::new(RefCell::new(values))))
            }
            ExprKind::ListLiteral(elements) | ExprKind::TupleLiteral(elements) => {
                let values = self.eval_arguments(elements, environment)?;
                if matches!(expression.kind, ExprKind::ListLiteral(_)) {
                    Ok(Value::List(Rc::new(RefCell::new(values))))
                } else {
                    Ok(Value::Tuple(values))
                }
            }
            ExprKind::Index { container, index } => {
                let container = self.eval(container, environment)?;
                if let ExprKind::Slice { start, end, step } = &index.kind {
                    let start = start
                        .as_deref()
                        .map(|value| self.eval(value, environment))
                        .transpose()?
                        .map(|value| number_value(value, position(expression)))
                        .transpose()?;
                    let end = end
                        .as_deref()
                        .map(|value| self.eval(value, environment))
                        .transpose()?
                        .map(|value| number_value(value, position(expression)))
                        .transpose()?;
                    let step = step
                        .as_deref()
                        .map(|value| self.eval(value, environment))
                        .transpose()?
                        .map(|value| number_value(value, position(expression)))
                        .transpose()?;
                    slice_value(container, start, end, step, position(expression))
                } else {
                    let index = self.eval(index, environment)?;
                    self.get_index(container, index, position(expression))
                }
            }
            ExprKind::Slice { .. } => Err(runtime_error(
                position(expression),
                "slice is only valid inside an index expression",
            )),
            ExprKind::ListComprehension {
                element,
                variable,
                iterable,
                condition,
            } => {
                let iterable = self.eval(iterable, environment)?;
                let mut values = Vec::new();
                for item in into_iteration(iterable, position(expression))? {
                    let scope = Scope::new(Some(environment.clone()));
                    Scope::define(&scope, variable.clone(), item);
                    if condition
                        .as_ref()
                        .map(|condition| self.eval(condition, &scope).map(|value| value.truthy()))
                        .transpose()?
                        .unwrap_or(true)
                    {
                        values.push(self.eval(element, &scope)?);
                    }
                }
                Ok(Value::List(Rc::new(RefCell::new(values))))
            }
            ExprKind::DictComprehension {
                key,
                value,
                variable,
                iterable,
                condition,
            } => {
                let iterable = self.eval(iterable, environment)?;
                let mut entries = Vec::new();
                for item in into_iteration(iterable, position(expression))? {
                    let scope = Scope::new(Some(environment.clone()));
                    Scope::define(&scope, variable.clone(), item);
                    if condition
                        .as_ref()
                        .map(|condition| self.eval(condition, &scope).map(|value| value.truthy()))
                        .transpose()?
                        .unwrap_or(true)
                    {
                        let key = self.eval(key, &scope)?;
                        ensure_hashable(&key, position(expression))?;
                        let value = self.eval(value, &scope)?;
                        dict_insert(&mut entries, key, value);
                    }
                }
                Ok(Value::Dict(Rc::new(RefCell::new(entries))))
            }
            ExprKind::Lambda { parameters, body } => {
                Ok(Value::Callable(Rc::new(Callable::Lambda {
                    parameters: parameters.clone(),
                    body: (**body).clone(),
                    closure: environment.clone(),
                })))
            }
            ExprKind::Help { argument } => {
                let value = self.eval(argument, environment)?;
                self.output.push(value.to_string());
                Ok(Value::Null)
            }
            ExprKind::Docstring { value } => Ok(Value::String(value.clone())),
            ExprKind::Unary { operator, operand } => {
                let value = self.eval(operand, environment)?;
                match operator {
                    UnaryOperator::Not => Ok(Value::Boolean(!value.truthy())),
                    UnaryOperator::Negate => match value {
                        Value::Number(number) => number
                            .checked_neg()
                            .map(Value::Number)
                            .ok_or_else(|| runtime_error(position(expression), "integer overflow")),
                        _ => Err(runtime_error(
                            position(expression),
                            "unary `-` requires a number",
                        )),
                    },
                }
            }
            ExprKind::Binary {
                operator,
                left,
                right,
            } => self.eval_binary(*operator, left, right, environment, position(expression)),
            ExprKind::Grouping(inner) => self.eval(inner, environment),
        }
    }

    fn eval_binary(
        &mut self,
        operator: BinaryOperator,
        left: &Expr,
        right: &Expr,
        environment: &Env,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        let left = self.eval(left, environment)?;
        if operator == BinaryOperator::And && !left.truthy() {
            return Ok(Value::Boolean(false));
        }
        if operator == BinaryOperator::Or && left.truthy() {
            return Ok(Value::Boolean(true));
        }
        let right = self.eval(right, environment)?;
        match operator {
            BinaryOperator::And => Ok(Value::Boolean(right.truthy())),
            BinaryOperator::Or => Ok(Value::Boolean(right.truthy())),
            BinaryOperator::Equal => Ok(Value::Boolean(values_equal(&left, &right))),
            BinaryOperator::NotEqual => Ok(Value::Boolean(!values_equal(&left, &right))),
            BinaryOperator::Add => match (left, right) {
                (Value::Number(a), Value::Number(b)) => checked_number(a.checked_add(b), at),
                (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
                (Value::List(a), Value::List(b)) => {
                    let mut values = a.borrow().clone();
                    values.extend(b.borrow().iter().cloned());
                    Ok(Value::List(Rc::new(RefCell::new(values))))
                }
                (Value::Tuple(mut a), Value::Tuple(b)) => {
                    a.extend(b);
                    Ok(Value::Tuple(a))
                }
                _ => Err(runtime_error(
                    at,
                    "`+` operands must be matching numbers, strings, or containers",
                )),
            },
            BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::Greater
            | BinaryOperator::GreaterEqual
            | BinaryOperator::Less
            | BinaryOperator::LessEqual => self.eval_numeric_binary(operator, left, right, at),
        }
    }

    fn eval_numeric_binary(
        &self,
        operator: BinaryOperator,
        left: Value,
        right: Value,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        let (Value::Number(a), Value::Number(b)) = (left, right) else {
            return Err(runtime_error(
                at,
                "arithmetic and comparison operators require numbers",
            ));
        };
        match operator {
            BinaryOperator::Subtract => checked_number(a.checked_sub(b), at),
            BinaryOperator::Multiply => checked_number(a.checked_mul(b), at),
            BinaryOperator::Divide => {
                if b == 0 {
                    Err(runtime_error(at, "division by zero"))
                } else {
                    checked_number(a.checked_div(b), at)
                }
            }
            BinaryOperator::Modulo => {
                if b == 0 {
                    Err(runtime_error(at, "modulo by zero"))
                } else {
                    checked_number(a.checked_rem(b), at)
                }
            }
            BinaryOperator::Greater => Ok(Value::Boolean(a > b)),
            BinaryOperator::GreaterEqual => Ok(Value::Boolean(a >= b)),
            BinaryOperator::Less => Ok(Value::Boolean(a < b)),
            BinaryOperator::LessEqual => Ok(Value::Boolean(a <= b)),
            _ => unreachable!("operator handled by caller"),
        }
    }

    fn eval_arguments(
        &mut self,
        expressions: &[Expr],
        environment: &Env,
    ) -> Result<Vec<Value>, RuntimeError> {
        expressions
            .iter()
            .map(|expression| self.eval(expression, environment))
            .collect()
    }

    fn call_value(
        &mut self,
        value: Value,
        arguments: Vec<Value>,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        match value {
            Value::Callable(callable) => self.call((*callable).clone(), arguments, at),
            Value::StructType(definition) => self.construct_struct(definition, arguments, at),
            Value::ClassType(definition) => self.construct_class(definition, arguments, at),
            Value::EnumType(definition) => Err(runtime_error(
                at,
                format!("enum `{}` requires a variant constructor", definition.name),
            )),
            other => Err(runtime_error(
                at,
                format!("{} is not callable", other.type_name()),
            )),
        }
    }

    fn call(
        &mut self,
        callable: Callable,
        arguments: Vec<Value>,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        match callable {
            Callable::Native(native) => self.call_native(native, arguments, at),
            Callable::BoundNative { receiver, method } => {
                self.call_method(receiver, &method, arguments, at)
            }
            Callable::UserFunction {
                function,
                closure,
                bound_self,
            } => {
                let environment = Scope::new(Some(closure.unwrap_or_else(|| self.globals.clone())));
                if let Some(instance) = bound_self {
                    Scope::define(&environment, "self", instance);
                }
                self.bind_parameters(&environment, &function.parameters, arguments, at)?;
                match self.execute_block(&function.body, &environment)? {
                    Flow::Return(value) => Ok(value),
                    Flow::Stop => Ok(Value::Null),
                    Flow::Break => Err(runtime_error(at, "`break` escaped a function")),
                    Flow::Continue => Err(runtime_error(at, "`continue` escaped a function")),
                    Flow::Normal => Ok(Value::Null),
                }
            }
            Callable::Lambda {
                parameters,
                body,
                closure,
            } => {
                let environment = Scope::new(Some(closure));
                self.bind_parameters(&environment, &parameters, arguments, at)?;
                self.eval(&body, &environment)
            }
        }
    }

    fn bind_parameters(
        &mut self,
        environment: &Env,
        parameters: &[Parameter],
        arguments: Vec<Value>,
        at: (usize, usize),
    ) -> Result<(), RuntimeError> {
        let mut arguments = arguments.into_iter();
        for parameter in parameters {
            if parameter.variadic {
                Scope::define(
                    environment,
                    parameter.name.clone(),
                    Value::List(Rc::new(RefCell::new(arguments.collect()))),
                );
                return Ok(());
            }
            let value = if let Some(value) = arguments.next() {
                value
            } else if let Some(default) = &parameter.default_value {
                self.eval(default, environment)?
            } else {
                return Err(runtime_error(
                    at,
                    format!("missing argument `{}`", parameter.name),
                ));
            };
            Scope::define(environment, parameter.name.clone(), value);
        }
        if arguments.next().is_some() {
            return Err(runtime_error(at, "too many function arguments"));
        }
        Ok(())
    }

    fn call_native(
        &mut self,
        function: NativeFunction,
        arguments: Vec<Value>,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        match function {
            NativeFunction::Len => {
                expect_argument_count(&arguments, 1, at)?;
                let length = match &arguments[0] {
                    Value::List(values) => values.borrow().len(),
                    Value::Tuple(values) => values.len(),
                    Value::Dict(values) => values.borrow().len(),
                    Value::String(value) => value.chars().count(),
                    Value::Range { start, stop, step } => range_len(*start, *stop, *step),
                    value => {
                        return Err(runtime_error(
                            at,
                            format!("len() is not defined for {}", value.type_name()),
                        ))
                    }
                };
                Ok(Value::Number(length as i64))
            }
            NativeFunction::Range => {
                if !(1..=3).contains(&arguments.len()) {
                    return Err(runtime_error(at, "range() expects one to three arguments"));
                }
                let numbers = arguments
                    .iter()
                    .map(|value| number_value(value.clone(), at))
                    .collect::<Result<Vec<_>, _>>()?;
                let (start, stop, step) = match numbers.as_slice() {
                    [stop] => (0, *stop, 1),
                    [start, stop] => (*start, *stop, 1),
                    [start, stop, step] => (*start, *stop, *step),
                    _ => unreachable!(),
                };
                if step == 0 {
                    return Err(runtime_error(at, "range() step cannot be zero"));
                }
                Ok(Value::Range { start, stop, step })
            }
            NativeFunction::Int => {
                expect_argument_count(&arguments, 1, at)?;
                match &arguments[0] {
                    Value::Number(number) => Ok(Value::Number(*number)),
                    Value::Boolean(value) => Ok(Value::Number(i64::from(*value))),
                    Value::String(value) => value
                        .parse::<i64>()
                        .map(Value::Number)
                        .map_err(|_| runtime_error(at, "int() requires an integer string")),
                    value => Err(runtime_error(
                        at,
                        format!("cannot convert {} to int", value.type_name()),
                    )),
                }
            }
            NativeFunction::Str => {
                expect_argument_count(&arguments, 1, at)?;
                Ok(Value::String(arguments[0].to_string()))
            }
            NativeFunction::StdWrite => {
                if !self.std_imported {
                    return Err(runtime_error(
                        at,
                        "std IO functions require `from library import std;`",
                    ));
                }
                expect_argument_count(&arguments, 1, at)?;
                Scope::assign(&self.globals, "a", arguments[0].clone());
                self.std_buffer = Some(arguments[0].clone());
                Ok(Value::Null)
            }
            NativeFunction::StdOutput => {
                if !self.std_imported {
                    return Err(runtime_error(
                        at,
                        "std IO functions require `from library import std;`",
                    ));
                }
                if arguments.len() > 1 {
                    return Err(runtime_error(
                        at,
                        "std.output() accepts zero or one argument",
                    ));
                }
                let value = self.std_buffer.clone().ok_or_else(|| {
                    runtime_error(
                        at,
                        "std.output() called before std.write() populated the global buffer",
                    )
                })?;
                self.output.push(format!("{value}\n"));
                Ok(Value::Null)
            }
            NativeFunction::StdPrintln => {
                if !self.std_imported {
                    return Err(runtime_error(
                        at,
                        "std IO functions require `from library import std;`",
                    ));
                }
                expect_argument_count(&arguments, 1, at)?;
                self.call_native(NativeFunction::StdWrite, arguments, at)?;
                self.call_native(NativeFunction::StdOutput, Vec::new(), at)
            }
        }
    }

    fn call_method(
        &mut self,
        receiver: Value,
        method: &str,
        arguments: Vec<Value>,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        match receiver.clone() {
            Value::StdLibrary => match method {
                "write" => self.call_native(NativeFunction::StdWrite, arguments, at),
                "output" => self.call_native(NativeFunction::StdOutput, arguments, at),
                "println" => self.call_native(NativeFunction::StdPrintln, arguments, at),
                _ => Err(runtime_error(
                    at,
                    format!("std library has no function `{method}`"),
                )),
            },
            Value::List(values) => {
                let mut values = values.borrow_mut();
                match method {
                    "append" => {
                        expect_argument_count(&arguments, 1, at)?;
                        values.push(arguments[0].clone());
                        Ok(Value::Null)
                    }
                    "pop" => {
                        if arguments.len() > 1 {
                            return Err(runtime_error(at, "pop() accepts zero or one argument"));
                        }
                        let index = arguments
                            .first()
                            .cloned()
                            .map(|value| number_value(value, at))
                            .transpose()?
                            .unwrap_or(-1);
                        let index = resolve_index(index, values.len(), at)?;
                        Ok(values.remove(index))
                    }
                    "remove" => {
                        expect_argument_count(&arguments, 1, at)?;
                        if let Some(index) = values
                            .iter()
                            .position(|value| values_equal(value, &arguments[0]))
                        {
                            values.remove(index);
                            Ok(Value::Null)
                        } else {
                            Err(runtime_error(at, "remove() value not found"))
                        }
                    }
                    "insert" => {
                        expect_argument_count(&arguments, 2, at)?;
                        let index = number_value(arguments[0].clone(), at)?;
                        let index = normalize_insert_index(index, values.len());
                        values.insert(index, arguments[1].clone());
                        Ok(Value::Null)
                    }
                    "clear" => {
                        expect_argument_count(&arguments, 0, at)?;
                        values.clear();
                        Ok(Value::Null)
                    }
                    _ => Err(runtime_error(at, format!("list has no method `{method}`"))),
                }
            }
            Value::Dict(entries) => {
                let mut entries = entries.borrow_mut();
                match method {
                    "keys" | "values" | "items" => {
                        expect_argument_count(&arguments, 0, at)?;
                        let values = entries
                            .iter()
                            .map(|(key, value)| match method {
                                "keys" => key.clone(),
                                "values" => value.clone(),
                                _ => Value::Tuple(vec![key.clone(), value.clone()]),
                            })
                            .collect();
                        Ok(Value::List(Rc::new(RefCell::new(values))))
                    }
                    "clear" => {
                        expect_argument_count(&arguments, 0, at)?;
                        entries.clear();
                        Ok(Value::Null)
                    }
                    _ => Err(runtime_error(at, format!("dict has no method `{method}`"))),
                }
            }
            Value::String(value) if method == "upper" || method == "lower" => {
                expect_argument_count(&arguments, 0, at)?;
                Ok(Value::String(if method == "upper" {
                    value.to_uppercase()
                } else {
                    value.to_lowercase()
                }))
            }
            Value::EnumType(definition) => {
                self.construct_enum_variant(&definition, method, arguments, at)
            }
            Value::ClassInstance(instance) => {
                let function = instance
                    .borrow()
                    .methods
                    .get(method)
                    .cloned()
                    .ok_or_else(|| runtime_error(at, format!("class has no method `{method}`")))?;
                self.call(
                    Callable::UserFunction {
                        function,
                        closure: None,
                        bound_self: Some(Value::ClassInstance(instance)),
                    },
                    arguments,
                    at,
                )
            }
            other => Err(runtime_error(
                at,
                format!("{} has no method `{method}`", other.type_name()),
            )),
        }
    }

    fn get_member(
        &self,
        object: Value,
        name: &str,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        match object {
            Value::StructInstance(instance) => instance
                .borrow()
                .fields
                .get(name)
                .cloned()
                .ok_or_else(|| runtime_error(at, format!("structure has no field `{name}`"))),
            Value::ClassInstance(instance) => {
                let (field, method) = {
                    let borrowed = instance.borrow();
                    (
                        borrowed.fields.get(name).cloned(),
                        borrowed.methods.get(name).cloned(),
                    )
                };
                if let Some(value) = field {
                    return Ok(value.clone());
                }
                if let Some(function) = method {
                    return Ok(Value::Callable(Rc::new(Callable::UserFunction {
                        function,
                        closure: None,
                        bound_self: Some(Value::ClassInstance(instance)),
                    })));
                }
                Err(runtime_error(at, format!("class has no member `{name}`")))
            }
            Value::EnumType(definition) => {
                let variant = definition
                    .variants
                    .iter()
                    .find(|variant| variant.name == name)
                    .ok_or_else(|| runtime_error(at, format!("enum has no variant `{name}`")))?;
                match variant.payload {
                    EnumVariantPayload::Unit => Ok(Value::EnumInstance(Box::new(EnumInstance {
                        name: definition.name,
                        variant: variant.name.clone(),
                        payload: EnumValuePayload::Unit,
                    }))),
                    _ => Err(runtime_error(
                        at,
                        format!("enum variant `{name}` requires payload arguments"),
                    )),
                }
            }
            Value::List(_) | Value::Dict(_) => {
                Ok(Value::Callable(Rc::new(Callable::BoundNative {
                    receiver: object,
                    method: name.to_owned(),
                })))
            }
            Value::EnumInstance(instance) if name == "variant" => {
                Ok(Value::String(instance.variant.clone()))
            }
            Value::EnumInstance(instance) => match &instance.payload {
                EnumValuePayload::Struct(fields) => fields.get(name).cloned().ok_or_else(|| {
                    runtime_error(at, format!("enum variant has no field `{name}`"))
                }),
                EnumValuePayload::Tuple(_) | EnumValuePayload::Unit => Err(runtime_error(
                    at,
                    format!("enum variant has no field `{name}`"),
                )),
            },
            other => Err(runtime_error(
                at,
                format!("{} has no member `{name}`", other.type_name()),
            )),
        }
    }

    fn set_member(
        &self,
        object: Value,
        name: &str,
        value: Value,
        at: (usize, usize),
    ) -> Result<(), RuntimeError> {
        match object {
            Value::StructInstance(instance) => {
                let mut instance = instance.borrow_mut();
                if !instance.fields.contains_key(name) {
                    return Err(runtime_error(
                        at,
                        format!("structure has no field `{name}`"),
                    ));
                }
                instance.fields.insert(name.to_owned(), value);
                Ok(())
            }
            Value::ClassInstance(instance) => {
                instance.borrow_mut().fields.insert(name.to_owned(), value);
                Ok(())
            }
            other => Err(runtime_error(
                at,
                format!("cannot assign a member on {}", other.type_name()),
            )),
        }
    }

    fn get_index(
        &self,
        container: Value,
        index: Value,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        match container {
            Value::List(values) => {
                let values = values.borrow();
                let index = resolve_index(number_value(index, at)?, values.len(), at)?;
                Ok(values[index].clone())
            }
            Value::Tuple(values) => {
                let index = resolve_index(number_value(index, at)?, values.len(), at)?;
                Ok(values[index].clone())
            }
            Value::String(value) => {
                let chars: Vec<_> = value.chars().collect();
                let index = resolve_index(number_value(index, at)?, chars.len(), at)?;
                Ok(Value::String(chars[index].to_string()))
            }
            Value::EnumInstance(instance) => match instance.payload {
                EnumValuePayload::Tuple(values) => {
                    let index = resolve_index(number_value(index, at)?, values.len(), at)?;
                    Ok(values[index].clone())
                }
                _ => Err(runtime_error(
                    at,
                    "enum variant does not have tuple payload",
                )),
            },
            Value::Dict(entries) => {
                let entries = entries.borrow();
                entries
                    .iter()
                    .find(|(key, _)| values_equal(key, &index))
                    .map(|(_, value)| value.clone())
                    .ok_or_else(|| runtime_error(at, format!("dictionary key {index} not found")))
            }
            other => Err(runtime_error(
                at,
                format!("{} does not support indexing", other.type_name()),
            )),
        }
    }

    fn set_index(
        &self,
        container: Value,
        index: Value,
        value: Value,
        at: (usize, usize),
    ) -> Result<(), RuntimeError> {
        match container {
            Value::List(values) => {
                let mut values = values.borrow_mut();
                let index = resolve_index(number_value(index, at)?, values.len(), at)?;
                values[index] = value;
                Ok(())
            }
            Value::Dict(entries) => {
                ensure_hashable(&index, at)?;
                dict_insert(&mut entries.borrow_mut(), index, value);
                Ok(())
            }
            Value::Tuple(_) => Err(runtime_error(at, "cannot assign to an immutable tuple")),
            other => Err(runtime_error(
                at,
                format!("{} does not support indexed assignment", other.type_name()),
            )),
        }
    }

    fn construct_struct(
        &mut self,
        definition: StructDefinition,
        arguments: Vec<Value>,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != definition.fields.len() {
            return Err(runtime_error(
                at,
                format!(
                    "{} expects {} constructor arguments, received {}",
                    definition.name,
                    definition.fields.len(),
                    arguments.len()
                ),
            ));
        }
        let fields = definition
            .fields
            .iter()
            .zip(arguments)
            .map(|(field, value)| (field.name.clone(), value))
            .collect();
        Ok(Value::StructInstance(Rc::new(RefCell::new(
            StructInstance {
                name: definition.name,
                fields,
            },
        ))))
    }

    fn construct_enum_variant(
        &self,
        definition: &EnumDefinition,
        variant_name: &str,
        arguments: Vec<Value>,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        let variant = definition
            .variants
            .iter()
            .find(|variant| variant.name == variant_name)
            .ok_or_else(|| runtime_error(at, format!("enum has no variant `{variant_name}`")))?;
        let payload = match &variant.payload {
            EnumVariantPayload::Unit => {
                expect_argument_count(&arguments, 0, at)?;
                EnumValuePayload::Unit
            }
            EnumVariantPayload::Tuple(types) => {
                expect_argument_count(&arguments, types.len(), at)?;
                EnumValuePayload::Tuple(arguments)
            }
            EnumVariantPayload::Struct(fields) => {
                let values = if arguments.len() == 1 {
                    match arguments.into_iter().next().unwrap() {
                        Value::Dict(entries) => entries
                            .borrow()
                            .iter()
                            .filter_map(|(key, value)| match key {
                                Value::String(key) => Some((key.clone(), value.clone())),
                                _ => None,
                            })
                            .collect::<HashMap<_, _>>(),
                        _ => {
                            return Err(runtime_error(
                                at,
                                "named enum payload expects one dictionary argument",
                            ))
                        }
                    }
                } else {
                    return Err(runtime_error(
                        at,
                        "named enum payload expects one dictionary argument",
                    ));
                };
                for field in fields {
                    if !values.contains_key(&field.name) {
                        return Err(runtime_error(
                            at,
                            format!("missing enum payload field `{}`", field.name),
                        ));
                    }
                }
                EnumValuePayload::Struct(values)
            }
        };
        Ok(Value::EnumInstance(Box::new(EnumInstance {
            name: definition.name.clone(),
            variant: variant.name.clone(),
            payload,
        })))
    }

    fn construct_class(
        &mut self,
        definition: ClassDefinition,
        arguments: Vec<Value>,
        at: (usize, usize),
    ) -> Result<Value, RuntimeError> {
        let methods: HashMap<_, _> = definition
            .members
            .iter()
            .filter_map(|member| match member {
                ClassMember::Function(function) => Some((function.name.clone(), function.clone())),
                ClassMember::Assignment(_) => None,
            })
            .collect();
        let instance = Rc::new(RefCell::new(ClassInstance {
            name: definition.name.clone(),
            fields: HashMap::new(),
            methods,
        }));
        let value = Value::ClassInstance(instance.clone());
        for member in &definition.members {
            if let ClassMember::Assignment(assignment) = member {
                let field = match &assignment.target.kind {
                    AssignmentTargetKind::Variable(name) => name,
                    _ => &assignment.name,
                };
                let field_value = self.eval(&assignment.value, &self.globals.clone())?;
                instance
                    .borrow_mut()
                    .fields
                    .insert(field.clone(), field_value);
            }
        }
        let initializer = { instance.borrow().methods.get("init").cloned() };
        if let Some(initializer) = initializer {
            self.call(
                Callable::UserFunction {
                    function: initializer,
                    closure: None,
                    bound_self: Some(value.clone()),
                },
                arguments,
                at,
            )?;
        } else if !arguments.is_empty() {
            return Err(runtime_error(
                at,
                format!("{} accepts no constructor arguments", definition.name),
            ));
        }
        Ok(value)
    }

    fn tick(&mut self, at: (usize, usize)) -> Result<(), RuntimeError> {
        self.steps += 1;
        if self.steps > 1_000_000 {
            return Err(runtime_error(at, "execution step limit exceeded"));
        }
        Ok(())
    }
}

impl Value {
    fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::StdLibrary => true,
            Self::Boolean(value) => *value,
            Self::Number(value) => *value != 0,
            Self::String(value) => !value.is_empty(),
            Self::List(values) => !values.borrow().is_empty(),
            Self::Tuple(values) => !values.is_empty(),
            Self::Dict(values) => !values.borrow().is_empty(),
            Self::Range { start, stop, step } => range_len(*start, *stop, *step) > 0,
            _ => true,
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::StdLibrary => "std library",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Boolean(_) => "boolean",
            Self::List(_) => "list",
            Self::Tuple(_) => "tuple",
            Self::Dict(_) => "dict",
            Self::Range { .. } => "range",
            Self::StructType(_) => "structure type",
            Self::EnumType(_) => "enum type",
            Self::ClassType(_) => "class type",
            Self::StructInstance(_) => "structure instance",
            Self::EnumInstance(_) => "enum instance",
            Self::ClassInstance(_) => "class instance",
            Self::Callable(_) => "function",
        }
    }
}

fn into_iteration(value: Value, at: (usize, usize)) -> Result<Iteration, RuntimeError> {
    match value {
        Value::List(values) => Ok(Iteration::Values(values.borrow().clone().into_iter())),
        Value::Tuple(values) => Ok(Iteration::Values(values.into_iter())),
        Value::String(value) => Ok(Iteration::Values(
            value
                .chars()
                .map(|value| Value::String(value.to_string()))
                .collect::<Vec<_>>()
                .into_iter(),
        )),
        Value::Dict(values) => Ok(Iteration::Values(
            values
                .borrow()
                .iter()
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>()
                .into_iter(),
        )),
        Value::Range { start, stop, step } => Ok(Iteration::Range {
            current: start,
            stop,
            step,
        }),
        other => Err(runtime_error(
            at,
            format!("{} is not iterable", other.type_name()),
        )),
    }
}

fn number_value(value: Value, at: (usize, usize)) -> Result<i64, RuntimeError> {
    match value {
        Value::Number(value) => Ok(value),
        other => Err(runtime_error(
            at,
            format!("expected a number, got {}", other.type_name()),
        )),
    }
}

fn slice_value(
    container: Value,
    start: Option<i64>,
    end: Option<i64>,
    step: Option<i64>,
    at: (usize, usize),
) -> Result<Value, RuntimeError> {
    let step = step.unwrap_or(1);
    if step == 0 {
        return Err(runtime_error(at, "slice step cannot be zero"));
    }
    let (values, was_string, was_tuple) = match container {
        Value::List(values) => (values.borrow().clone(), false, false),
        Value::Tuple(values) => (values, false, true),
        Value::String(value) => (
            value
                .chars()
                .map(|character| Value::String(character.to_string()))
                .collect(),
            true,
            false,
        ),
        other => {
            return Err(runtime_error(
                at,
                format!("{} does not support slicing", other.type_name()),
            ))
        }
    };
    let length = values.len() as i64;
    let (mut current, limit) = if step > 0 {
        (
            start
                .map(|value| normalize_slice_bound(value, length, 0, length))
                .unwrap_or(0),
            end.map(|value| normalize_slice_bound(value, length, 0, length))
                .unwrap_or(length),
        )
    } else {
        (
            start
                .map(|value| normalize_slice_bound(value, length, -1, length - 1))
                .unwrap_or(length - 1),
            end.map(|value| normalize_slice_bound(value, length, -1, length - 1))
                .unwrap_or(-1),
        )
    };
    let mut sliced = Vec::new();
    while if step > 0 {
        current < limit
    } else {
        current > limit
    } {
        if let Some(value) = usize::try_from(current)
            .ok()
            .and_then(|index| values.get(index))
        {
            sliced.push(value.clone());
        }
        let Some(next) = current.checked_add(step) else {
            break;
        };
        current = next;
    }
    if was_string {
        Ok(Value::String(
            sliced.into_iter().map(|value| value.to_string()).collect(),
        ))
    } else if was_tuple {
        Ok(Value::Tuple(sliced))
    } else {
        Ok(Value::List(Rc::new(RefCell::new(sliced))))
    }
}

fn normalize_slice_bound(bound: i64, length: i64, minimum: i64, maximum: i64) -> i64 {
    let adjusted = if bound < 0 {
        bound.saturating_add(length)
    } else {
        bound
    };
    adjusted.clamp(minimum, maximum)
}

fn resolve_index(index: i64, length: usize, at: (usize, usize)) -> Result<usize, RuntimeError> {
    let resolved = if index < 0 {
        (length as i64).checked_add(index)
    } else {
        Some(index)
    };
    resolved
        .and_then(|index| usize::try_from(index).ok())
        .filter(|index| *index < length)
        .ok_or_else(|| {
            runtime_error(
                at,
                format!("index {index} out of range for length {length}"),
            )
        })
}

fn normalize_insert_index(index: i64, length: usize) -> usize {
    if index < 0 {
        (length as i64 + index).clamp(0, length as i64) as usize
    } else {
        index.min(length as i64) as usize
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Number(left), Value::Number(right)) => left == right,
        (Value::String(left), Value::String(right)) => left == right,
        (Value::Boolean(left), Value::Boolean(right)) => left == right,
        (Value::List(left), Value::List(right)) => {
            let left = left.borrow();
            let right = right.borrow();
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(a, b)| values_equal(a, b))
        }
        (Value::Tuple(left), Value::Tuple(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(a, b)| values_equal(a, b))
        }
        (Value::EnumInstance(left), Value::EnumInstance(right)) => {
            left.name == right.name && left.variant == right.variant
        }
        _ => false,
    }
}

fn dict_insert(entries: &mut Vec<(Value, Value)>, key: Value, value: Value) {
    if let Some((_, existing)) = entries
        .iter_mut()
        .find(|(candidate, _)| values_equal(candidate, &key))
    {
        *existing = value;
    } else {
        entries.push((key, value));
    }
}

fn ensure_hashable(value: &Value, at: (usize, usize)) -> Result<(), RuntimeError> {
    match value {
        Value::Number(_) | Value::String(_) | Value::Boolean(_) => Ok(()),
        Value::Tuple(values) => {
            for value in values {
                ensure_hashable(value, at)?;
            }
            Ok(())
        }
        other => Err(runtime_error(
            at,
            format!("{} cannot be used as a dictionary key", other.type_name()),
        )),
    }
}

fn range_len(start: i64, stop: i64, step: i64) -> usize {
    if (step > 0 && start >= stop) || (step < 0 && start <= stop) {
        return 0;
    }
    let distance = if step > 0 {
        (stop as i128) - (start as i128)
    } else {
        (start as i128) - (stop as i128)
    };
    let magnitude = (step as i128).abs();
    ((distance + magnitude - 1) / magnitude).min(usize::MAX as i128) as usize
}

fn expect_argument_count(
    arguments: &[Value],
    expected: usize,
    at: (usize, usize),
) -> Result<(), RuntimeError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(runtime_error(
            at,
            format!(
                "expected {expected} arguments, received {}",
                arguments.len()
            ),
        ))
    }
}

fn checked_number(value: Option<i64>, at: (usize, usize)) -> Result<Value, RuntimeError> {
    value
        .map(Value::Number)
        .ok_or_else(|| runtime_error(at, "integer overflow"))
}

fn position<T>(node: &T) -> (usize, usize)
where
    T: Positioned,
{
    (node.line(), node.column())
}

trait Positioned {
    fn line(&self) -> usize;
    fn column(&self) -> usize;
}

impl Positioned for Expr {
    fn line(&self) -> usize {
        self.line
    }
    fn column(&self) -> usize {
        self.column
    }
}

impl Positioned for AssignmentTarget {
    fn line(&self) -> usize {
        self.line
    }
    fn column(&self) -> usize {
        self.column
    }
}

fn statement_position(statement: &Statement) -> (usize, usize) {
    match statement {
        Statement::Assignment(node) => (node.line, node.column),
        Statement::Expression(node) => (node.line, node.column),
        Statement::If(node) => (node.line, node.column),
        Statement::While(node) => (node.line, node.column),
        Statement::Return(node) => (node.line, node.column),
        Statement::Stop(node) => (node.line, node.column),
        Statement::Rules(node) => (node.line, node.column),
        Statement::Break(node) => (node.line, node.column),
        Statement::Continue(node) => (node.line, node.column),
        Statement::For(node) => (node.line, node.column),
        Statement::Try(node) => (node.line, node.column),
        Statement::Const(node) => (node.line, node.column),
        Statement::GcControl(node) => (node.line, node.column),
        Statement::Thread(node) => (node.line, node.column),
        Statement::Spawn(node) => (node.line, node.column),
        Statement::Join(node) => (node.line, node.column),
        Statement::Empty(node) => (node.line, node.column),
    }
}

fn runtime_error(at: (usize, usize), message: impl Into<String>) -> RuntimeError {
    RuntimeError {
        message: message.into(),
        line: at.0,
        column: at.1,
    }
}

pub fn execute(program: &Program) -> Result<RunResult, RuntimeError> {
    Interpreter::new().execute(program)
}

pub fn run_source(source: &str) -> Result<RunResult, RunError> {
    let program = easyscript_parser::parse(source).map_err(RunError::Parse)?;
    execute(&program).map_err(RunError::Runtime)
}

#[derive(Debug)]
pub enum RunError {
    Parse(easyscript_parser::ParseError),
    Runtime(RuntimeError),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(f),
            Self::Runtime(error) => error.fmt(f),
        }
    }
}

impl Error for RunError {}
