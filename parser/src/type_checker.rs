use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::ast::{
    AssignmentTargetKind, Block, ClassMember, Expr, ExprKind, Function, Program, Statement,
    TopLevelItem, UnaryOperator,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for TypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at line {}, column {}",
            self.message, self.line, self.column
        )
    }
}

impl Error for TypeError {}

pub fn validate_mutability(program: &Program) -> Result<(), TypeError> {
    for item in &program.items {
        match item {
            TopLevelItem::Function(function) => check_function(function)?,
            TopLevelItem::ExternFunction(_) | TopLevelItem::Stop(_) => {}
            TopLevelItem::Struct(_) | TopLevelItem::Enum(_) => {}
            TopLevelItem::Statement(statement) => check_statement(statement, &mut HashMap::new())?,
            TopLevelItem::Class(class) => {
                for member in &class.members {
                    match member {
                        ClassMember::Function(function) => check_function(function)?,
                        ClassMember::Assignment(assignment) => {
                            check_statement(
                                &Statement::Assignment(assignment.clone()),
                                &mut HashMap::new(),
                            )?;
                        }
                    }
                }
            }
            TopLevelItem::Header(_)
            | TopLevelItem::Import(_)
            | TopLevelItem::EspkgPackage(_)
            | TopLevelItem::TypeAlias(_) => {}
        }
    }
    Ok(())
}

fn check_function(function: &Function) -> Result<(), TypeError> {
    let mut local_variables = HashMap::new();
    for parameter in &function.parameters {
        if let Some(default_value) = &parameter.default_value {
            check_expression(default_value, &local_variables)?;
        }
    }
    check_block(&function.body, &mut local_variables)?;
    Ok(())
}

fn check_block(
    block: &Block,
    variables: &mut HashMap<String, ContainerType>,
) -> Result<(), TypeError> {
    for statement in &block.statements {
        check_statement(statement, variables)?;
    }
    Ok(())
}

fn check_statement(
    statement: &Statement,
    variables: &mut HashMap<String, ContainerType>,
) -> Result<(), TypeError> {
    match statement {
        Statement::Assignment(assignment) => {
            check_expression(&assignment.value, variables)?;
            match &assignment.target.kind {
                AssignmentTargetKind::Variable(name) => {
                    if let Some(shape) = infer_container_type(&assignment.value, variables) {
                        variables.insert(name.clone(), shape);
                    } else {
                        variables.remove(name);
                    }
                }
                AssignmentTargetKind::Index { container, index } => {
                    check_expression(container, variables)?;
                    check_expression(index, variables)?;
                    if may_be_tuple(container, variables) {
                        return Err(type_error(
                            assignment.target.line,
                            assignment.target.column,
                            "cannot assign to a tuple element; tuples are immutable",
                        ));
                    }
                }
                AssignmentTargetKind::Member { object, .. } => {
                    check_expression(object, variables)?;
                }
            }
        }
        Statement::Expression(expression) => {
            check_expression(&expression.expression, variables)?;
        }
        Statement::If(if_statement) => {
            for (condition, block) in &if_statement.branches {
                check_expression(condition, variables)?;
                let mut branch_variables = variables.clone();
                check_block(block, &mut branch_variables)?;
            }
            if let Some(block) = &if_statement.else_block {
                let mut branch_variables = variables.clone();
                check_block(block, &mut branch_variables)?;
            }
        }
        Statement::While(while_statement) => {
            check_expression(&while_statement.condition, variables)?;
            let mut loop_variables = variables.clone();
            check_block(&while_statement.body, &mut loop_variables)?;
        }
        Statement::Return(return_statement) => {
            check_expression(&return_statement.value, variables)?;
        }
        Statement::Rules(rules_statement) => {
            if let Some(value) = &rules_statement.value {
                check_expression(value, variables)?;
            }
            if let Some(block) = &rules_statement.body {
                let mut rule_variables = variables.clone();
                check_block(block, &mut rule_variables)?;
            }
        }
        Statement::For(for_statement) => {
            check_expression(&for_statement.iterable, variables)?;
            let mut loop_variables = variables.clone();
            loop_variables.remove(&for_statement.variable);
            check_block(&for_statement.body, &mut loop_variables)?;
        }
        Statement::Try(try_statement) => {
            let mut try_variables = variables.clone();
            check_block(&try_statement.body, &mut try_variables)?;
            for handler in &try_statement.handlers {
                let mut handler_variables = variables.clone();
                check_block(&handler.body, &mut handler_variables)?;
            }
            if let Some(block) = &try_statement.else_block {
                let mut else_variables = variables.clone();
                check_block(block, &mut else_variables)?;
            }
            if let Some(block) = &try_statement.finally_block {
                let mut finally_variables = variables.clone();
                check_block(block, &mut finally_variables)?;
            }
        }
        Statement::Const(constant) => {
            check_statement(
                &Statement::Assignment(constant.assignment.clone()),
                variables,
            )?;
        }
        Statement::Thread(thread) => {
            let mut thread_variables = variables.clone();
            check_block(&thread.body, &mut thread_variables)?;
        }
        Statement::GcControl(_) | Statement::Spawn(_) | Statement::Join(_) => {}
        Statement::Stop(_) | Statement::Break(_) | Statement::Continue(_) => {}
        Statement::Empty(_) => {}
    }
    Ok(())
}

fn check_expression(
    expression: &Expr,
    variables: &HashMap<String, ContainerType>,
) -> Result<(), TypeError> {
    match &expression.kind {
        ExprKind::Call { arguments, .. } => {
            for argument in arguments {
                check_expression(argument, variables)?;
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            arguments,
        } => {
            check_expression(receiver, variables)?;
            for argument in arguments {
                check_expression(argument, variables)?;
            }
            if is_mutating_method(method) && may_be_tuple(receiver, variables) {
                return Err(type_error(
                    expression.line,
                    expression.column,
                    format!("cannot call mutating method `{method}` on a tuple"),
                ));
            }
        }
        ExprKind::Member { object, .. } => check_expression(object, variables)?,
        ExprKind::DictLiteral(entries) => {
            for (key, value) in entries {
                check_expression(key, variables)?;
                check_expression(value, variables)?;
            }
        }
        ExprKind::ListLiteral(elements) | ExprKind::TupleLiteral(elements) => {
            for element in elements {
                check_expression(element, variables)?;
            }
        }
        ExprKind::Index { container, index } => {
            check_expression(container, variables)?;
            check_expression(index, variables)?;
        }
        ExprKind::Slice { start, end, step } => {
            for bound in [start, end, step].into_iter().flatten() {
                check_expression(bound, variables)?;
            }
        }
        ExprKind::ListComprehension {
            element,
            iterable,
            condition,
            ..
        } => {
            check_expression(element, variables)?;
            check_expression(iterable, variables)?;
            if let Some(condition) = condition {
                check_expression(condition, variables)?;
            }
        }
        ExprKind::DictComprehension {
            key,
            value,
            iterable,
            condition,
            ..
        } => {
            check_expression(key, variables)?;
            check_expression(value, variables)?;
            check_expression(iterable, variables)?;
            if let Some(condition) = condition {
                check_expression(condition, variables)?;
            }
        }
        ExprKind::Lambda { parameters, body } => {
            for parameter in parameters {
                if let Some(value) = &parameter.default_value {
                    check_expression(value, variables)?;
                }
            }
            check_expression(body, variables)?;
        }
        ExprKind::Help { argument } => check_expression(argument, variables)?,
        ExprKind::Unary { operand, .. } | ExprKind::Grouping(operand) => {
            check_expression(operand, variables)?;
        }
        ExprKind::Binary { left, right, .. } => {
            check_expression(left, variables)?;
            check_expression(right, variables)?;
        }
        ExprKind::Number(_)
        | ExprKind::String(_)
        | ExprKind::Boolean(_)
        | ExprKind::Identifier(_)
        | ExprKind::Docstring { .. } => {}
    }
    Ok(())
}

fn is_mutating_method(method: &str) -> bool {
    matches!(method, "append" | "pop" | "remove" | "insert" | "clear")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContainerType {
    List(Vec<Option<ContainerType>>),
    Tuple(Vec<Option<ContainerType>>),
    Dictionary(Vec<(Expr, Option<ContainerType>)>),
}

fn infer_container_type(
    expression: &Expr,
    variables: &HashMap<String, ContainerType>,
) -> Option<ContainerType> {
    match &expression.kind {
        ExprKind::ListLiteral(elements) => Some(ContainerType::List(
            elements
                .iter()
                .map(|element| infer_container_type(element, variables))
                .collect(),
        )),
        ExprKind::TupleLiteral(elements) => Some(ContainerType::Tuple(
            elements
                .iter()
                .map(|element| infer_container_type(element, variables))
                .collect(),
        )),
        ExprKind::DictLiteral(entries) => Some(ContainerType::Dictionary(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), infer_container_type(value, variables)))
                .collect(),
        )),
        ExprKind::DictComprehension { .. } => Some(ContainerType::Dictionary(Vec::new())),
        ExprKind::ListComprehension { element, .. } => {
            Some(ContainerType::List(vec![infer_container_type(
                element, variables,
            )]))
        }
        ExprKind::Identifier(name) => variables.get(name).cloned(),
        ExprKind::Grouping(inner) => infer_container_type(inner, variables),
        ExprKind::Index { container, index } => {
            if matches!(&index.kind, ExprKind::Slice { .. }) {
                return infer_container_type(container, variables);
            }
            let container_type = infer_container_type(container, variables)?;
            let elements = match container_type {
                ContainerType::List(elements) | ContainerType::Tuple(elements) => elements,
                ContainerType::Dictionary(entries) => {
                    if let Some(key) = literal_key(index) {
                        return entries
                            .iter()
                            .find(|(candidate, _)| same_literal_key(candidate, &key))
                            .and_then(|(_, value)| value.clone());
                    }
                    let mut known = entries.iter().filter_map(|(_, value)| value.as_ref());
                    let first = known.next()?.clone();
                    return known.all(|value| *value == first).then_some(first);
                }
            };
            if let Some(index) = literal_index(index) {
                let resolved = if index < 0 {
                    elements.len().checked_sub(index.unsigned_abs() as usize)?
                } else {
                    index as usize
                };
                elements.get(resolved)?.clone()
            } else {
                let mut known = elements.iter().flatten();
                let first = known.next()?.clone();
                known.all(|element| *element == first).then_some(first)
            }
        }
        ExprKind::Call { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::Member { .. }
        | ExprKind::Slice { .. }
        | ExprKind::Lambda { .. }
        | ExprKind::Help { .. }
        | ExprKind::Docstring { .. }
        | ExprKind::Number(_)
        | ExprKind::String(_)
        | ExprKind::Boolean(_)
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. } => None,
    }
}

fn may_be_tuple(expression: &Expr, variables: &HashMap<String, ContainerType>) -> bool {
    possible_container_types(expression, variables)
        .iter()
        .any(|container| matches!(container, ContainerType::Tuple(_)))
}

fn possible_container_types(
    expression: &Expr,
    variables: &HashMap<String, ContainerType>,
) -> Vec<ContainerType> {
    match &expression.kind {
        ExprKind::ListLiteral(elements) => vec![ContainerType::List(
            elements
                .iter()
                .map(|element| infer_container_type(element, variables))
                .collect(),
        )],
        ExprKind::TupleLiteral(elements) => vec![ContainerType::Tuple(
            elements
                .iter()
                .map(|element| infer_container_type(element, variables))
                .collect(),
        )],
        ExprKind::DictLiteral(entries) => vec![ContainerType::Dictionary(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), infer_container_type(value, variables)))
                .collect(),
        )],
        ExprKind::DictComprehension { .. } => vec![ContainerType::Dictionary(Vec::new())],
        ExprKind::ListComprehension { element, .. } => {
            vec![ContainerType::List(vec![infer_container_type(
                element, variables,
            )])]
        }
        ExprKind::Identifier(name) => variables.get(name).cloned().into_iter().collect(),
        ExprKind::Grouping(inner) => possible_container_types(inner, variables),
        ExprKind::Index { container, index } => {
            if matches!(&index.kind, ExprKind::Slice { .. }) {
                return possible_container_types(container, variables);
            }
            let literal_index = literal_index(index);
            possible_container_types(container, variables)
                .into_iter()
                .flat_map(|container| {
                    let elements = match container {
                        ContainerType::List(elements) | ContainerType::Tuple(elements) => elements,
                        ContainerType::Dictionary(entries) => {
                            let selected: Vec<_> = if let Some(key) = literal_key(index) {
                                entries
                                    .iter()
                                    .filter(|(candidate, _)| same_literal_key(candidate, &key))
                                    .map(|(_, value)| value)
                                    .collect()
                            } else {
                                entries.iter().map(|(_, value)| value).collect()
                            };
                            return selected.into_iter().flatten().cloned().collect();
                        }
                    };
                    let selected: Vec<_> = if let Some(index) = literal_index {
                        let resolved = if index < 0 {
                            elements.len().checked_sub(index.unsigned_abs() as usize)
                        } else {
                            Some(index as usize)
                        };
                        resolved
                            .and_then(|index| elements.get(index))
                            .into_iter()
                            .collect()
                    } else {
                        elements.iter().collect()
                    };
                    selected.into_iter().flatten().cloned().collect::<Vec<_>>()
                })
                .collect()
        }
        ExprKind::Member { object, .. } => possible_container_types(object, variables),
        ExprKind::Call { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::Slice { .. }
        | ExprKind::Lambda { .. }
        | ExprKind::Help { .. }
        | ExprKind::Docstring { .. }
        | ExprKind::Number(_)
        | ExprKind::String(_)
        | ExprKind::Boolean(_)
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. } => Vec::new(),
    }
}

fn literal_index(expression: &Expr) -> Option<i64> {
    match &expression.kind {
        ExprKind::Number(number) => number.parse().ok(),
        ExprKind::Unary {
            operator: UnaryOperator::Negate,
            operand,
        } => literal_index(operand)?.checked_neg(),
        ExprKind::Grouping(inner) => literal_index(inner),
        _ => None,
    }
}

fn literal_key(expression: &Expr) -> Option<Expr> {
    match &expression.kind {
        ExprKind::Number(_) | ExprKind::String(_) => Some(expression.clone()),
        ExprKind::TupleLiteral(elements) => Some(Expr {
            line: expression.line,
            column: expression.column,
            kind: ExprKind::TupleLiteral(
                elements
                    .iter()
                    .map(literal_key)
                    .collect::<Option<Vec<_>>>()?,
            ),
        }),
        _ => None,
    }
}

fn same_literal_key(left: &Expr, right: &Expr) -> bool {
    match (&left.kind, &right.kind) {
        (ExprKind::Number(left), ExprKind::Number(right))
        | (ExprKind::String(left), ExprKind::String(right)) => left == right,
        (ExprKind::TupleLiteral(left), ExprKind::TupleLiteral(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same_literal_key(left, right))
        }
        _ => false,
    }
}

fn type_error(line: usize, column: usize, message: impl Into<String>) -> TypeError {
    TypeError {
        message: message.into(),
        line,
        column,
    }
}
