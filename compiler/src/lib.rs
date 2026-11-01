use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use easyscript_parser::ast::{
    AssignmentTargetKind, BinaryOperator, Expr, ExprKind, Function, Program, Statement,
    TopLevelItem, UnaryOperator,
};

pub fn compile_to_elf(program: &Program, output_path: &Path) -> Result<(), String> {
    let rust_source = generate_rust(program)?;
    let temporary_output = output_path.with_extension(format!("tmp-{}", std::process::id()));
    let result = Command::new("rustc")
        .arg("--edition=2021")
        .arg("--target=x86_64-unknown-linux-gnu")
        .arg("-O")
        .arg("-o")
        .arg(&temporary_output)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = result.map_err(|error| format!("Rust compiler error: {error}"))?;
    let write_result = child
        .stdin
        .take()
        .expect("piped rustc stdin")
        .write_all(rust_source.as_bytes());
    if let Err(error) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "Rust compiler error: could not provide generated source: {error}"
        ));
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("Rust compiler error: {error}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(&temporary_output);
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Rust compiler error:\n{diagnostics}"));
    }

    if let Err(error) = fs::rename(&temporary_output, output_path) {
        let _ = fs::remove_file(&temporary_output);
        return Err(format!(
            "Build error: could not install {}: {error}",
            output_path.display()
        ));
    }
    Ok(())
}

fn generate_rust(program: &Program) -> Result<String, String> {
    let mut functions = Vec::new();
    let mut imported_std = false;
    for item in &program.items {
        match item {
            TopLevelItem::Function(function) => functions.push(function),
            TopLevelItem::Header(_) | TopLevelItem::Stop(_) => {}
            TopLevelItem::Import(import) if import.library == "std" => imported_std = true,
            TopLevelItem::Import(import) => {
                return Err(format!(
                    "EasyScript AOT error: library `{}` is not supported at line {}, column {}",
                    import.library, import.line, import.column
                ))
            }
            other => {
                let (line, column) = top_level_position(other);
                return Err(unsupported("this top-level declaration", line, column));
            }
        }
    }

    let function_indices: HashMap<_, _> = functions
        .iter()
        .enumerate()
        .map(|(index, function)| (function.name.as_str(), index))
        .collect();
    let entry_index = functions
        .iter()
        .position(|function| function.is_entry)
        .ok_or_else(|| "EasyScript AOT error: missing `int main()` entry point".to_owned())?;

    for function in &functions {
        if let Some(parameter) = function
            .parameters
            .iter()
            .find(|parameter| parameter.variadic)
        {
            return Err(unsupported(
                "variadic parameters",
                parameter.line,
                parameter.column,
            ));
        }
        validate_block(&function.body, imported_std, &function_indices)?;
    }

    let mut source = String::from(RUST_RUNTIME);
    for (index, function) in functions.iter().enumerate() {
        write_function(
            &mut source,
            index,
            function,
            &function_indices,
            imported_std,
        )?;
    }
    source.push_str("fn call(name: &str, args: Vec<Value>) -> Value {\n    match name {\n");
    for (index, function) in functions.iter().enumerate() {
        writeln!(
            source,
            "        {:?} => es_function_{index}(args),",
            function.name
        )
        .expect("writing to String cannot fail");
    }
    source.push_str(
        "        _ => panic!(\"undefined function `{}`\", name),\n    }\n}\n\
         fn main() {\n\
             let result = es_function_",
    );
    write!(source, "{entry_index}(Vec::new());\n").expect("writing to String cannot fail");
    source.push_str(
        "    let exit_code = match result { Value::Number(code) => code as i32, _ => 0 };\n\
             std::process::exit(exit_code);\n\
         }\n",
    );
    Ok(source)
}

fn validate_block(
    block: &easyscript_parser::Block,
    imported_std: bool,
    functions: &HashMap<&str, usize>,
) -> Result<(), String> {
    for statement in &block.statements {
        match statement {
            Statement::Assignment(assignment) => {
                if !matches!(assignment.target.kind, AssignmentTargetKind::Variable(_)) {
                    return Err(unsupported(
                        "member/index assignment",
                        assignment.line,
                        assignment.column,
                    ));
                }
                validate_expr(&assignment.value, imported_std, functions)?;
            }
            Statement::Expression(expression) => {
                validate_expr(&expression.expression, imported_std, functions)?
            }
            Statement::If(statement) => {
                for (condition, branch) in &statement.branches {
                    validate_expr(condition, imported_std, functions)?;
                    validate_block(branch, imported_std, functions)?;
                }
                if let Some(branch) = &statement.else_block {
                    validate_block(branch, imported_std, functions)?;
                }
            }
            Statement::Return(statement) => {
                validate_expr(&statement.value, imported_std, functions)?
            }
            Statement::Rules(statement) => {
                if !imported_std {
                    return Err(format!(
                        "EasyScript AOT error: `rules!` IO commands require `from library import std;` at line {}, column {}",
                        statement.line, statement.column
                    ));
                }
                if let Some(value) = &statement.value {
                    validate_expr(value, imported_std, functions)?;
                }
            }
            Statement::Empty(_) => {}
            other => {
                let (line, column) = statement_position(other);
                return Err(unsupported("this statement", line, column));
            }
        }
    }
    Ok(())
}

fn validate_expr(
    expression: &Expr,
    imported_std: bool,
    functions: &HashMap<&str, usize>,
) -> Result<(), String> {
    match &expression.kind {
        ExprKind::Number(_) | ExprKind::String(_) | ExprKind::Boolean(_) => {}
        ExprKind::Identifier(_) => {}
        ExprKind::Call { callee, arguments } => {
            let valid_std_function = matches!(callee.as_str(), "write" | "output");
            if !functions.contains_key(callee.as_str()) && !valid_std_function {
                return Err(format!(
                    "EasyScript AOT error: function `{callee}` is not defined at line {}, column {}",
                    expression.line, expression.column
                ));
            }
            if valid_std_function && !functions.contains_key(callee.as_str()) {
                validate_std_call(callee, arguments.len(), imported_std, expression)?;
            }
            for argument in arguments {
                validate_expr(argument, imported_std, functions)?;
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            arguments,
        } if matches!(&receiver.kind, ExprKind::Identifier(name) if name == "std") => {
            validate_std_call(method, arguments.len(), imported_std, expression)?;
            for argument in arguments {
                validate_expr(argument, imported_std, functions)?;
            }
        }
        ExprKind::Unary { operand, .. } => validate_expr(operand, imported_std, functions)?,
        ExprKind::Binary { left, right, .. } => {
            validate_expr(left, imported_std, functions)?;
            validate_expr(right, imported_std, functions)?;
        }
        ExprKind::Grouping(inner) => validate_expr(inner, imported_std, functions)?,
        _ => {
            return Err(unsupported(
                "this expression",
                expression.line,
                expression.column,
            ))
        }
    }
    Ok(())
}

fn validate_std_call(
    name: &str,
    argument_count: usize,
    imported_std: bool,
    expression: &Expr,
) -> Result<(), String> {
    if !imported_std {
        return Err(format!(
            "EasyScript AOT error: std IO functions require `from library import std;` at line {}, column {}",
            expression.line, expression.column
        ));
    }
    let valid_count = match name {
        "write" => argument_count == 1,
        "output" => argument_count <= 1,
        _ => false,
    };
    if !valid_count {
        return Err(format!(
            "EasyScript AOT error: invalid arguments to `std.{name}()` at line {}, column {}",
            expression.line, expression.column
        ));
    }
    Ok(())
}

fn write_function(
    source: &mut String,
    index: usize,
    function: &Function,
    functions: &HashMap<&str, usize>,
    imported_std: bool,
) -> Result<(), String> {
    writeln!(
        source,
        "fn es_function_{index}(args: Vec<Value>) -> Value {{\n\
             let mut env = HashMap::<String, Value>::new();\n\
             if args.len() > {} {{ panic!(\"too many arguments to `{}`\"); }}",
        function.parameters.len(),
        function.name
    )
    .expect("writing to String cannot fail");
    if imported_std {
        source.push_str("env.insert(\"std\".to_owned(), Value::Std);\n");
    }
    for (parameter_index, parameter) in function.parameters.iter().enumerate() {
        let default = if let Some(default_value) = &parameter.default_value {
            format!("{{ {} }}", emit_expr(default_value, functions)?)
        } else {
            format!(
                "panic!(\"missing argument `{}` to `{}`\")",
                parameter.name, function.name
            )
        };
        writeln!(
            source,
            "let value = args.get({parameter_index}).cloned().unwrap_or_else(|| {default});\n\
             env.insert({:?}.to_owned(), value);",
            parameter.name
        )
        .expect("writing to String cannot fail");
    }
    write_block(source, &function.body, functions)?;
    source.push_str("Value::Unit\n}\n");
    Ok(())
}

fn write_block(
    source: &mut String,
    block: &easyscript_parser::Block,
    functions: &HashMap<&str, usize>,
) -> Result<(), String> {
    for statement in &block.statements {
        match statement {
            Statement::Assignment(assignment) => {
                let AssignmentTargetKind::Variable(name) = &assignment.target.kind else {
                    return Err(unsupported(
                        "member/index assignment",
                        assignment.line,
                        assignment.column,
                    ));
                };
                writeln!(
                    source,
                    "env.insert({name:?}.to_owned(), {});",
                    emit_expr(&assignment.value, functions)?
                )
                .expect("writing to String cannot fail");
            }
            Statement::Expression(expression) => {
                writeln!(source, "{};", emit_expr(&expression.expression, functions)?)
                    .expect("writing to String cannot fail");
            }
            Statement::If(statement) => {
                for (index, (condition, branch)) in statement.branches.iter().enumerate() {
                    if index == 0 {
                        write!(
                            source,
                            "if truthy(&{}) {{\n",
                            emit_expr(condition, functions)?
                        )
                    } else {
                        write!(
                            source,
                            "else if truthy(&{}) {{\n",
                            emit_expr(condition, functions)?
                        )
                    }
                    .expect("writing to String cannot fail");
                    write_block(source, branch, functions)?;
                    source.push_str("}\n");
                }
                if let Some(branch) = &statement.else_block {
                    source.push_str("else {\n");
                    write_block(source, branch, functions)?;
                    source.push_str("}\n");
                }
            }
            Statement::Return(statement) => {
                writeln!(
                    source,
                    "return {};",
                    emit_expr(&statement.value, functions)?
                )
                .expect("writing to String cannot fail");
            }
            Statement::Rules(statement) => {
                let operation = statement.operation.as_deref().unwrap_or_default();
                let value = statement.value.as_ref().ok_or_else(|| {
                    unsupported(
                        "incomplete `rules!` command",
                        statement.line,
                        statement.column,
                    )
                })?;
                let expression = emit_expr(value, functions)?;
                match operation {
                    "write" => writeln!(
                        source,
                        "{{ let value = {expression}; env.insert(\"a\".to_owned(), value.clone()); easyscript_std::write(value); }}"
                    ),
                    "output" => {
                        writeln!(source, "{{ let _ = {expression}; easyscript_std::output(); }}")
                    }
                    _ => {
                        return Err(unsupported(
                            "this `rules!` operation",
                            statement.line,
                            statement.column,
                        ))
                    }
                }
                .expect("writing to String cannot fail");
            }
            Statement::Empty(_) => {}
            other => {
                let (line, column) = statement_position(other);
                return Err(unsupported("this statement", line, column));
            }
        }
    }
    Ok(())
}

fn emit_expr(expression: &Expr, functions: &HashMap<&str, usize>) -> Result<String, String> {
    let generated = match &expression.kind {
        ExprKind::Number(number) => format!(
            "Value::Number({number:?}.parse::<i64>().expect(\"invalid integer literal\"))"
        ),
        ExprKind::String(value) => format!("Value::String({value:?}.to_owned())"),
        ExprKind::Boolean(value) => format!("Value::Boolean({value})"),
        ExprKind::Identifier(name) => format!("get(&env, {name:?})"),
        ExprKind::Call { callee, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| emit_expr(argument, functions))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            if functions.contains_key(callee.as_str()) {
                format!("call({callee:?}, vec![{arguments}])")
            } else {
                match callee.as_str() {
                    "write" => format!("easyscript_std::write({arguments})"),
                    "output" if arguments.is_empty() => "easyscript_std::output()".to_owned(),
                    "output" => {
                        format!("{{ let _ = {arguments}; easyscript_std::output() }}")
                    }
                    _ => {
                        return Err(format!(
                            "EasyScript AOT error: function `{callee}` is not defined at line {}, column {}",
                            expression.line, expression.column
                        ))
                    }
                }
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            arguments,
        } if matches!(&receiver.kind, ExprKind::Identifier(name) if name == "std") => {
            let arguments = arguments
                .iter()
                .map(|argument| emit_expr(argument, functions))
                .collect::<Result<Vec<_>, _>>()?;
            match method.as_str() {
                "write" => format!("easyscript_std::write({})", arguments[0]),
                "output" if arguments.is_empty() => "easyscript_std::output()".to_owned(),
                "output" => format!(
                    "{{ let _ = {}; easyscript_std::output() }}",
                    arguments[0]
                ),

                _ => return Err(unsupported("this std function", expression.line, expression.column)),
            }
        }
        ExprKind::Unary { operator, operand } => match operator {
            UnaryOperator::Negate => format!("negate({})", emit_expr(operand, functions)?),
            UnaryOperator::Not => format!("Value::Boolean(!truthy(&{}))", emit_expr(operand, functions)?),
        },
        ExprKind::Binary {
            operator: BinaryOperator::And,
            left,
            right,
        } => format!(
            "{{ let left = {}; if truthy(&left) {{ Value::Boolean(truthy(&{})) }} else {{ Value::Boolean(false) }} }}",
            emit_expr(left, functions)?,
            emit_expr(right, functions)?
        ),
        ExprKind::Binary {
            operator: BinaryOperator::Or,
            left,
            right,
        } => format!(
            "{{ let left = {}; if truthy(&left) {{ Value::Boolean(true) }} else {{ Value::Boolean(truthy(&{})) }} }}",
            emit_expr(left, functions)?,
            emit_expr(right, functions)?
        ),
        ExprKind::Binary {
            operator,
            left,
            right,
        } => format!(
            "binary({}, {}, {:?})",
            emit_expr(left, functions)?,
            emit_expr(right, functions)?,
            binary_operator(*operator)
        ),
        ExprKind::Grouping(inner) => emit_expr(inner, functions)?,
        _ => return Err(unsupported("this expression", expression.line, expression.column)),
    };
    Ok(generated)
}

fn binary_operator(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "add",
        BinaryOperator::Subtract => "subtract",
        BinaryOperator::Multiply => "multiply",
        BinaryOperator::Divide => "divide",
        BinaryOperator::Modulo => "modulo",
        BinaryOperator::GreaterEqual => "greater_equal",
        BinaryOperator::Greater => "greater",
        BinaryOperator::LessEqual => "less_equal",
        BinaryOperator::Less => "less",
        BinaryOperator::Equal => "equal",
        BinaryOperator::NotEqual => "not_equal",
        BinaryOperator::And | BinaryOperator::Or => {
            unreachable!("short-circuit operators handled above")
        }
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

fn top_level_position(item: &TopLevelItem) -> (usize, usize) {
    match item {
        TopLevelItem::Header(node) => (node.line, node.column),
        TopLevelItem::Import(node) => (node.line, node.column),
        TopLevelItem::EspkgPackage(node) => (node.line, node.column),
        TopLevelItem::Function(node) => (node.line, node.column),
        TopLevelItem::Struct(node) => (node.line, node.column),
        TopLevelItem::Enum(node) => (node.line, node.column),
        TopLevelItem::ExternFunction(node) => (node.line, node.column),
        TopLevelItem::Statement(node) => statement_position(node),
        TopLevelItem::Class(node) => (node.line, node.column),
        TopLevelItem::TypeAlias(node) => (node.line, node.column),
        TopLevelItem::Stop(node) => (node.line, node.column),
    }
}

fn unsupported(feature: &str, line: usize, column: usize) -> String {
    format!("EasyScript AOT error: {feature} is not supported at line {line}, column {column}")
}

const RUST_RUNTIME: &str = r#"
use std::collections::HashMap;
use std::sync::Mutex;

static STD_BUFFER: Mutex<Option<Value>> = Mutex::new(None);

#[derive(Clone, Debug)]
enum Value {
    Unit,
    Std,
    Number(i64),
    String(String),
    Boolean(bool),
}

impl std::fmt::Display for Value {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Unit => formatter.write_str("None"),
            Value::Std => formatter.write_str("<std library>"),
            Value::Number(value) => write!(formatter, "{value}"),
            Value::String(value) => formatter.write_str(value),
            Value::Boolean(value) => write!(formatter, "{value}"),
        }
    }
}

mod easyscript_std {
    use super::{Value, STD_BUFFER};

    pub fn write(value: Value) -> Value {
        *STD_BUFFER.lock().expect("std buffer lock poisoned") = Some(value);
        Value::Unit
    }

    pub fn output() -> Value {
        let value = STD_BUFFER
            .lock()
            .expect("std buffer lock poisoned")
            .clone()
            .expect("std.output() called before std.write() populated the global buffer");
        println!("{value}");
        Value::Unit
    }

}

fn get(environment: &HashMap<String, Value>, name: &str) -> Value {
    environment.get(name).cloned().unwrap_or_else(|| panic!("undefined variable `{name}`"))
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Unit => false,
        Value::Std => true,
        Value::Number(value) => *value != 0,
        Value::String(value) => !value.is_empty(),
        Value::Boolean(value) => *value,
    }
}

fn negate(value: Value) -> Value {
    match value {
        Value::Number(number) => Value::Number(number.checked_neg().expect("integer overflow")),
        other => panic!("cannot negate {other}"),
    }
}

fn binary(left: Value, right: Value, operator: &str) -> Value {
    match (left, right, operator) {
        (Value::Number(left), Value::Number(right), "add") =>
            Value::Number(left.checked_add(right).expect("integer overflow")),
        (Value::String(left), Value::String(right), "add") =>
            Value::String(left + &right),
        (Value::Number(left), Value::Number(right), "subtract") =>
            Value::Number(left.checked_sub(right).expect("integer overflow")),
        (Value::Number(left), Value::Number(right), "multiply") =>
            Value::Number(left.checked_mul(right).expect("integer overflow")),
        (Value::Number(_), Value::Number(0), "divide" | "modulo") =>
            panic!("division or modulo by zero"),
        (Value::Number(left), Value::Number(right), "divide") =>
            Value::Number(left.checked_div(right).expect("integer overflow")),
        (Value::Number(left), Value::Number(right), "modulo") =>
            Value::Number(left.checked_rem(right).expect("integer overflow")),
        (Value::Number(left), Value::Number(right), "greater") => Value::Boolean(left > right),
        (Value::Number(left), Value::Number(right), "greater_equal") => Value::Boolean(left >= right),
        (Value::Number(left), Value::Number(right), "less") => Value::Boolean(left < right),
        (Value::Number(left), Value::Number(right), "less_equal") => Value::Boolean(left <= right),
        (left, right, "equal") => Value::Boolean(equal(&left, &right)),
        (left, right, "not_equal") => Value::Boolean(!equal(&left, &right)),
        (left, right, operator) => panic!("invalid operands for {operator}: {left}, {right}"),
    }
}

fn equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Unit, Value::Unit) | (Value::Std, Value::Std) => true,
        (Value::Number(left), Value::Number(right)) => left == right,
        (Value::String(left), Value::String(right)) => left == right,
        (Value::Boolean(left), Value::Boolean(right)) => left == right,
        _ => false,
    }
}
"#;
