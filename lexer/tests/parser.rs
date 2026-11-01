use easyscript::ast::{BinaryOperator, ExprKind, Statement, TopLevelItem};
use easyscript::{parse, validate_mutability, TokenKind};

const IF_ELSE: &str = r#"int main() {
  a = int -> 40;
  b = int -> 30;
  if (a >= b) {
    println("Hello World");
  } else {
    println("a is not bigger than b!");
  }
}
stop;
"#;

const WHILE_TRUE: &str = r#"int main() {
    a = int -> 0;
    b = int -> 1;
    while (True) {
        println(a + 1);
        println(b + 2);
    }
}
stop;
"#;

const WHILE_FALSE: &str = r#"int main() {
    a = int -> 10;
    while (False) {
        println("这段永远不会执行");
    }
}
stop;
"#;

const ELIF_REQUIRED_SEMICOLONS: &str = r#"int main() {
    a = int -> 10;
    b = int -> 5;
    if (a + b == 15) {
        println("True");
    } elif (a + b + 5 == 20) {
        println("True again!");
    } else {
        println("False :(");
    }
}
"#;

const BREAK_CONTINUE_REQUIRED_SEMICOLONS: &str = r#"int main() {
    i = int -> 0;
    while (True) {
        i = int -> i + 1;
        if (i == 5) {
            continue;
        }
        if (i == 10) {
            break;
        }
        println(i);
    }
}
"#;

const LOGICAL_OPERATORS: &str = r#"int main() {
    x = int -> 20;
    y = int -> 8;
    if (x > 10 && y < 10) {
        println("ok");
    }
    if (!(x == 0) || y > 100) {
        println("or test");
    }
}
"#;

#[test]
fn parses_if_else_sample_and_positions() {
    let program = parse(IF_ELSE).unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    assert_eq!((main.line, main.column), (1, 1));
    assert_eq!(main.name, "main");
    assert!(main.is_entry);
    assert_eq!(main.body.statements.len(), 3);

    let Statement::If(if_statement) = &main.body.statements[2] else {
        panic!("expected if statement");
    };
    assert_eq!((if_statement.line, if_statement.column), (4, 3));
    assert_eq!(if_statement.branches.len(), 1);
    assert_eq!(if_statement.branches[0].1.statements.len(), 1);
    assert_eq!(
        if_statement.else_block.as_ref().unwrap().statements.len(),
        1
    );

    let ExprKind::Binary {
        operator,
        left,
        right,
    } = &if_statement.branches[0].0.kind
    else {
        panic!("expected comparison expression");
    };
    assert_eq!(*operator, BinaryOperator::GreaterEqual);
    assert_eq!(
        (
            if_statement.branches[0].0.line,
            if_statement.branches[0].0.column
        ),
        (4, 9)
    );
    assert_eq!(left.kind, ExprKind::Identifier("a".to_owned()));
    assert_eq!(right.kind, ExprKind::Identifier("b".to_owned()));
}

#[test]
fn parses_while_true_and_keeps_body_as_syntax() {
    let program = parse(WHILE_TRUE).unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::While(loop_statement) = &main.body.statements[2] else {
        panic!("expected while statement");
    };

    assert_eq!(loop_statement.condition.kind, ExprKind::Boolean(true));
    assert_eq!(loop_statement.body.statements.len(), 2);
}

#[test]
fn parses_while_false_without_executing_or_discarding_its_body() {
    let program = parse(WHILE_FALSE).unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::While(loop_statement) = &main.body.statements[1] else {
        panic!("expected while statement");
    };

    assert_eq!(loop_statement.condition.kind, ExprKind::Boolean(false));
    assert_eq!(loop_statement.body.statements.len(), 1);
    let Statement::Expression(expression) = &loop_statement.body.statements[0] else {
        panic!("expected println expression");
    };
    let ExprKind::Call { arguments, .. } = &expression.expression.kind else {
        panic!("expected function call");
    };
    assert_eq!(
        arguments[0].kind,
        ExprKind::String("这段永远不会执行".to_owned())
    );
}

#[test]
fn parses_elif_chains_and_else_with_required_semicolons() {
    let program = parse(ELIF_REQUIRED_SEMICOLONS).unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    assert_eq!(main.body.statements.len(), 3);

    let Statement::If(if_statement) = &main.body.statements[2] else {
        panic!("expected if statement");
    };
    assert_eq!(if_statement.branches.len(), 2);
    assert!(if_statement.else_block.is_some());
    assert_eq!(if_statement.branches[0].0.line, 4);
    assert_eq!(if_statement.branches[1].0.line, 6);
    assert_eq!(if_statement.branches[0].1.statements.len(), 1);
    assert_eq!(if_statement.branches[1].1.statements.len(), 1);
    assert_eq!(
        if_statement.else_block.as_ref().unwrap().statements.len(),
        1
    );
}

#[test]
fn parses_break_and_continue_in_loop_with_mixed_terminators() {
    let program = parse(BREAK_CONTINUE_REQUIRED_SEMICOLONS).unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::While(loop_statement) = &main.body.statements[1] else {
        panic!("expected while statement");
    };
    assert_eq!(loop_statement.body.statements.len(), 4);
    let Statement::If(continue_if) = &loop_statement.body.statements[1] else {
        panic!("expected if containing continue");
    };
    assert!(matches!(
        continue_if.branches[0].1.statements[0],
        Statement::Continue(_)
    ));
    let Statement::If(break_if) = &loop_statement.body.statements[2] else {
        panic!("expected if containing break");
    };
    assert!(matches!(
        break_if.branches[0].1.statements[0],
        Statement::Break(_)
    ));
}

#[test]
fn rejects_break_and_continue_outside_a_loop_with_positions() {
    for (source, keyword) in [("break", "`break`"), ("continue", "`continue`")] {
        let error = parse(&format!("int main() {{ {source}; }}")).unwrap_err();
        assert_eq!((error.line, error.column), (1, 14));
        assert!(error.message.contains(keyword));
        assert!(error.message.contains("loop"));
    }
}

#[test]
fn parses_logical_operators_with_the_requested_precedence() {
    let program = parse(LOGICAL_OPERATORS).unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::If(and_if) = &main.body.statements[2] else {
        panic!("expected first if statement");
    };
    let ExprKind::Binary {
        operator: BinaryOperator::And,
        left,
        right,
    } = &and_if.branches[0].0.kind
    else {
        panic!("&& should bind less tightly than comparisons");
    };
    assert!(matches!(
        left.kind,
        ExprKind::Binary {
            operator: BinaryOperator::Greater,
            ..
        }
    ));
    assert!(matches!(
        right.kind,
        ExprKind::Binary {
            operator: BinaryOperator::Less,
            ..
        }
    ));

    let Statement::If(or_if) = &main.body.statements[3] else {
        panic!("expected second if statement");
    };
    let ExprKind::Binary {
        operator: BinaryOperator::Or,
        left,
        right,
    } = &or_if.branches[0].0.kind
    else {
        panic!("|| should be the lowest-precedence operator");
    };
    assert!(matches!(
        left.kind,
        ExprKind::Unary {
            operator: easyscript::ast::UnaryOperator::Not,
            ..
        }
    ));
    assert!(matches!(
        right.kind,
        ExprKind::Binary {
            operator: BinaryOperator::Greater,
            ..
        }
    ));
}

#[test]
fn logical_not_binds_tighter_than_arithmetic_and_or_binds_lowest() {
    let program = parse("int main() { if (!1 * 2 + 3 >= 4 && True || False) {} }").unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::If(if_statement) = &main.body.statements[0] else {
        panic!("expected if statement");
    };
    let ExprKind::Binary {
        operator: BinaryOperator::Or,
        left,
        ..
    } = &if_statement.branches[0].0.kind
    else {
        panic!("|| should be the root of the expression");
    };
    let ExprKind::Binary {
        operator: BinaryOperator::And,
        left,
        ..
    } = &left.kind
    else {
        panic!("&& should bind more tightly than ||");
    };
    let ExprKind::Binary {
        operator: BinaryOperator::GreaterEqual,
        left,
        ..
    } = &left.kind
    else {
        panic!("comparison should bind more tightly than &&");
    };
    let ExprKind::Binary {
        operator: BinaryOperator::Add,
        left,
        ..
    } = &left.kind
    else {
        panic!("addition should bind more tightly than comparison");
    };
    let ExprKind::Binary {
        operator: BinaryOperator::Multiply,
        left,
        ..
    } = &left.kind
    else {
        panic!("multiplication should bind more tightly than addition");
    };
    assert!(matches!(
        left.kind,
        ExprKind::Unary {
            operator: easyscript::ast::UnaryOperator::Not,
            ..
        }
    ));
}

#[test]
fn parses_each_comparison_operator_and_optional_else() {
    let source = r#"int main() {
  if (1 >= 0) {}
  if (1 > 0) {}
  if (1 <= 2) {}
  if (1 < 2) {}
  if (1 == 1) {}
  if (1 != 2) {}
}
"#;
    let program = parse(source).unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let operators: Vec<_> = main
        .body
        .statements
        .iter()
        .map(|statement| {
            let Statement::If(if_statement) = statement else {
                panic!("expected if statement");
            };
            assert!(if_statement.else_block.is_none());
            let ExprKind::Binary { operator, .. } = &if_statement.branches[0].0.kind else {
                panic!("expected comparison condition");
            };
            *operator
        })
        .collect();
    assert_eq!(
        operators,
        vec![
            BinaryOperator::GreaterEqual,
            BinaryOperator::Greater,
            BinaryOperator::LessEqual,
            BinaryOperator::Less,
            BinaryOperator::Equal,
            BinaryOperator::NotEqual,
        ]
    );
}

#[test]
fn parses_declarations_rules_and_expression_precedence() {
    let source = r#"active system linux;
from library import std;
def calc(a=int, b=auto) -> int {
  value = int -> 1 + 2 * 3 >= 4;
  return code -> 0;
}
int main() {
  rules! output -> 1 + 2;
}
stop;
"#;
    let program = parse(source).unwrap();
    assert!(matches!(program.items[0], TopLevelItem::Header(_)));
    assert!(matches!(program.items[1], TopLevelItem::Import(_)));

    let TopLevelItem::Function(function) = &program.items[2] else {
        panic!("expected function definition");
    };
    assert_eq!(function.name, "calc");
    assert_eq!(function.parameters.len(), 2);
    assert_eq!(function.parameters[0].ty.name, "int");
    assert_eq!(function.parameters[1].ty.name, "auto");

    let Statement::Assignment(assignment) = &function.body.statements[0] else {
        panic!("expected assignment");
    };
    let ExprKind::Binary {
        operator: BinaryOperator::GreaterEqual,
        left,
        ..
    } = &assignment.value.kind
    else {
        panic!("comparison should be the outer expression");
    };
    let ExprKind::Binary {
        operator: BinaryOperator::Add,
        right,
        ..
    } = &left.kind
    else {
        panic!("addition should bind more tightly than comparison");
    };
    assert!(matches!(
        right.kind,
        ExprKind::Binary {
            operator: BinaryOperator::Multiply,
            ..
        }
    ));
    let TopLevelItem::Function(main) = &program.items[3] else {
        panic!("expected main entry");
    };
    let Statement::Rules(rules) = &main.body.statements[0] else {
        panic!("expected rules command");
    };
    assert_eq!(rules.operation.as_deref(), Some("output"));
    assert!(matches!(
        rules.value.as_ref().unwrap().kind,
        ExprKind::Binary {
            operator: BinaryOperator::Add,
            ..
        }
    ));
}

#[test]
fn requires_one_main_only_declarations_and_semicolon_terminated_simple_statements() {
    let missing_main = parse("def helper() {}\n").unwrap_err();
    assert!(missing_main
        .message
        .contains("missing its required `int main()`"));

    let duplicate_main = parse("int main() {}\nint main() {}\n").unwrap_err();
    assert!(duplicate_main.message.contains("only one `int main()`"));

    let top_level_code = parse("println(\"not in main\");\nint main() {}\n").unwrap_err();
    assert_eq!((top_level_code.line, top_level_code.column), (1, 1));
    assert!(top_level_code.message.contains("only"));

    let missing_terminator =
        parse("int main() {\n    println(\"missing semicolon\")\n}\n").unwrap_err();
    assert_eq!((missing_terminator.line, missing_terminator.column), (3, 1));
    assert!(missing_terminator.message.contains("expected `;`"));

    let misplaced_stop = parse("int main() {}\nstop;\ndef later() {}\n").unwrap_err();
    assert!(misplaced_stop.message.contains("final source marker"));

    let inner_stop = parse("int main() { stop; }").unwrap_err();
    assert!(inner_stop.message.contains("end of the source"));
}

#[test]
fn permits_the_final_stop_marker_and_accepts_library_declarations_before_main() {
    let program = parse(
        "active system linux;\nfrom library import std;\ndef helper() { return code -> 1; }\nint main() {}\nstop;\n",
    )
    .unwrap();
    assert!(matches!(program.items.last(), Some(TopLevelItem::Stop(_))));
}

#[test]
fn preserves_large_integer_literals_in_the_ast() {
    let program =
        parse("int main() { number = int -> 999999999999999999999999999999999999; }").unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::Assignment(assignment) = &main.body.statements[0] else {
        panic!("expected assignment");
    };
    assert_eq!(
        assignment.value.kind,
        ExprKind::Number("999999999999999999999999999999999999".to_owned())
    );
}

#[test]
fn parses_lists_tuples_nesting_indexes_methods_and_len() {
    let source = r#"int main() {
    arr = auto -> [1, 2, "test", True];
    nest = auto -> [ (1,2), [3,4] ];
    arr[0] = int -> 9;
    last = auto -> arr[-1];
    pair = auto -> (10, 20, 30);
    item = auto -> pair[-1];
    arr.append("more");
    arr.pop();
    arr.remove("test");
    arr.insert(0, (5, 6));
    arr.clear();
    size = int -> len(arr);
    println(nest[0][1]);
}
"#;
    let program = parse(source).unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    assert_eq!(main.body.statements.len(), 13);

    let Statement::Assignment(list_assignment) = &main.body.statements[0] else {
        panic!("expected list assignment");
    };
    assert!(matches!(
        list_assignment.value.kind,
        ExprKind::ListLiteral(ref elements) if elements.len() == 4
    ));
    assert_eq!(
        (list_assignment.value.line, list_assignment.value.column),
        (2, 19)
    );

    let Statement::Assignment(nested_assignment) = &main.body.statements[1] else {
        panic!("expected nested container assignment");
    };
    let ExprKind::ListLiteral(elements) = &nested_assignment.value.kind else {
        panic!("expected outer list literal");
    };
    assert!(matches!(elements[0].kind, ExprKind::TupleLiteral(_)));
    assert!(matches!(elements[1].kind, ExprKind::ListLiteral(_)));

    let Statement::Assignment(index_assignment) = &main.body.statements[2] else {
        panic!("expected indexed assignment");
    };
    assert!(matches!(
        index_assignment.target.kind,
        easyscript::ast::AssignmentTargetKind::Index { .. }
    ));
    assert_eq!(
        (index_assignment.target.line, index_assignment.target.column),
        (4, 8)
    );

    let Statement::Assignment(last_assignment) = &main.body.statements[3] else {
        panic!("expected index read assignment");
    };
    assert!(matches!(last_assignment.value.kind, ExprKind::Index { .. }));
    let Statement::Assignment(tuple_assignment) = &main.body.statements[4] else {
        panic!("expected tuple assignment");
    };
    assert!(matches!(
        tuple_assignment.value.kind,
        ExprKind::TupleLiteral(ref elements) if elements.len() == 3
    ));

    for (statement, expected_method) in main.body.statements[6..11]
        .iter()
        .zip(["append", "pop", "remove", "insert", "clear"])
    {
        let Statement::Expression(expression) = statement else {
            panic!("expected method call statement");
        };
        assert!(matches!(
            &expression.expression.kind,
            ExprKind::MethodCall { method, .. } if method == expected_method
        ));
    }

    let Statement::Assignment(size_assignment) = &main.body.statements[11] else {
        panic!("expected len assignment");
    };
    assert!(matches!(
        &size_assignment.value.kind,
        ExprKind::Call { callee, arguments }
            if callee == "len" && arguments.len() == 1
    ));

    validate_mutability(&program).unwrap();
}

#[test]
fn type_check_rejects_tuple_element_writes_including_nested_tuples() {
    for (source, expected_line) in [
        (
            "int main() {\n t = auto -> (10, 20);\n t[0] = int -> 1;\n}\n",
            3,
        ),
        (
            "int main() {\n nest = auto -> [(1, 2), [3, 4]];\n nest[0][1] = int -> 9;\n}\n",
            3,
        ),
        (
            "int main() {\n nest = auto -> [(1, 2), [3, 4]];\n nest[unknown][0] = int -> 9;\n}\n",
            3,
        ),
    ] {
        let program = parse(source).unwrap();
        let error = validate_mutability(&program).unwrap_err();
        assert_eq!(error.line, expected_line);
        assert!(error.message.contains("tuples are immutable"));
    }
}

#[test]
fn type_check_rejects_mutating_tuple_methods_but_allows_list_methods() {
    let list_program = parse("int main() {\n xs = auto -> [1];\n xs.append(2);\n}\n").unwrap();
    validate_mutability(&list_program).unwrap();

    let tuple_program = parse("int main() {\n xs = auto -> (1, 2);\n xs.clear();\n}\n").unwrap();
    let error = validate_mutability(&tuple_program).unwrap_err();
    assert_eq!((error.line, error.column), (3, 4));
    assert!(error.message.contains("mutating method `clear`"));
}

#[test]
fn supports_empty_and_singleton_containers_and_mutable_values_inside_tuples() {
    let program = parse(
        "int main() {\n empty = auto -> [];\n no_items = auto -> ();\n one_item = auto -> (7,);\n pair = auto -> ([1], 2);\n pair[0].append(3);\n}\n",
    )
    .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::Assignment(empty) = &main.body.statements[0] else {
        panic!("expected empty list assignment");
    };
    assert!(matches!(empty.value.kind, ExprKind::ListLiteral(ref values) if values.is_empty()));
    let Statement::Assignment(no_items) = &main.body.statements[1] else {
        panic!("expected empty tuple assignment");
    };
    assert!(matches!(
        no_items.value.kind,
        ExprKind::TupleLiteral(ref values) if values.is_empty()
    ));
    let Statement::Assignment(one_item) = &main.body.statements[2] else {
        panic!("expected singleton tuple assignment");
    };
    assert!(matches!(
        one_item.value.kind,
        ExprKind::TupleLiteral(ref values) if values.len() == 1
    ));
    validate_mutability(&program).unwrap();
}

#[test]
fn parses_original_hello_world_source() {
    let source = r#"active system linux;
from library import std;
def HelloWorld() -> int {
    return code -> "Hello, World!";
}
int main() {
    HelloWorld();
    rules! output -> "ready";
}
stop;
"#;
    assert!(parse(source).is_ok(), "{:?}", parse(source).unwrap_err());
}

#[test]
fn reports_parse_errors_with_line_and_column() {
    let error = parse("int main() {\n if (True { }\n}").unwrap_err();
    assert_eq!((error.line, error.column), (2, 11));
    assert!(error.message.contains("expected `)`"));
}

#[test]
fn preserves_lexer_errors_through_the_parser_api() {
    let error = parse("int main() {\n @\n}").unwrap_err();
    assert_eq!((error.line, error.column), (2, 2));
    assert!(error.message.contains("unexpected character"));
}

#[test]
fn lexer_still_recognizes_addition_sample_tokens() {
    let tokens = easyscript::lex("A+B").unwrap();
    assert_eq!(tokens[1].kind, TokenKind::Plus);
}

#[test]
fn parses_dictionaries_and_dictionary_values_keep_tuple_immutability() {
    let program = parse(
        r#"int main() {
    data = auto -> {"name":"es", "ver":1, (1, 2): (3, 4)};
    name = auto -> data["name"];
    data["ver"] = int -> 2;
}
"#,
    )
    .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::Assignment(assignment) = &main.body.statements[0] else {
        panic!("expected dictionary assignment");
    };
    let ExprKind::DictLiteral(entries) = &assignment.value.kind else {
        panic!("expected dictionary literal");
    };
    assert_eq!(entries.len(), 3);
    assert_eq!((assignment.value.line, assignment.value.column), (2, 20));
    validate_mutability(&program).unwrap();

    let tuple_value = parse(
        r#"int main() {
    data = auto -> {"pair": (1, 2)};
    data["pair"][0] = int -> 9;
}
"#,
    )
    .unwrap();
    let error = validate_mutability(&tuple_value).unwrap_err();
    assert_eq!(error.line, 3);
    assert!(error.message.contains("tuples are immutable"));
}

#[test]
fn parses_for_in_range_and_container_iteration_syntax() {
    let program = parse(
        r#"int main() {
    for i in range(0,10,2) {
        println(i);
    }
    nums = auto -> [100,200,300];
    for v in nums {
        println(v);
    }
}
"#,
    )
    .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::For(range_loop) = &main.body.statements[0] else {
        panic!("expected range for loop");
    };
    assert_eq!(range_loop.variable, "i");
    assert_eq!(
        (range_loop.variable_line, range_loop.variable_column),
        (2, 9)
    );
    assert!(matches!(
        range_loop.iterable.kind,
        ExprKind::Call { ref callee, ref arguments } if callee == "range" && arguments.len() == 3
    ));
    let Statement::For(list_loop) = &main.body.statements[2] else {
        panic!("expected list for loop");
    };
    assert_eq!(list_loop.variable, "v");
    assert!(matches!(
        list_loop.iterable.kind,
        ExprKind::Identifier(ref name) if name == "nums"
    ));
}

#[test]
fn parses_try_except_else_finally_without_executing_code() {
    let program = parse(
        r#"int main() {
    try {
        a = int -> 1 / 0;
    } except ZeroDivisionError {
        println("division error");
    } else {
        println("no error");
    } finally {
        println("cleanup");
    }
}
"#,
    )
    .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::Try(try_statement) = &main.body.statements[0] else {
        panic!("expected try statement");
    };
    assert_eq!(try_statement.handlers.len(), 1);
    assert_eq!(
        try_statement.handlers[0].error_type.as_deref(),
        Some("ZeroDivisionError")
    );
    assert!(try_statement.else_block.is_some());
    assert!(try_statement.finally_block.is_some());
    assert_eq!(try_statement.body.statements.len(), 1);
}

#[test]
fn parses_comprehensions_and_class_members_with_source_positions() {
    let program = parse(
        r#"int main() {
    evens = auto -> [x for x in range(10) if x % 2 == 0];
    squares = auto -> {x: x * x for x in range(10) if x > 2};
}
class Person {
    def init(name=str) {
        self.name = auto -> name;
    }
    def greet() {
        println(self.name);
    }
}
"#,
    )
    .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::Assignment(evens) = &main.body.statements[0] else {
        panic!("expected list comprehension assignment");
    };
    let ExprKind::ListComprehension {
        variable,
        condition,
        ..
    } = &evens.value.kind
    else {
        panic!("expected list comprehension");
    };
    assert_eq!(variable, "x");
    assert!(condition.is_some());
    assert_eq!((evens.value.line, evens.value.column), (2, 21));
    let Statement::Assignment(squares) = &main.body.statements[1] else {
        panic!("expected dictionary comprehension assignment");
    };
    assert!(matches!(
        squares.value.kind,
        ExprKind::DictComprehension { .. }
    ));

    let TopLevelItem::Class(person) = &program.items[1] else {
        panic!("expected class definition");
    };
    assert_eq!(person.name, "Person");
    assert_eq!(person.members.len(), 2);
    let easyscript::ast::ClassMember::Function(init) = &person.members[0] else {
        panic!("expected init method");
    };
    assert_eq!(init.parameters[0].ty.name, "str");
    assert!(matches!(
        person.members[1],
        easyscript::ast::ClassMember::Function(_)
    ));
}

#[test]
fn parses_string_slices_lambda_defaults_and_variadic_parameters() {
    let program = parse(
        r#"int main() {
    text = auto -> "abcdef";
    char = auto -> text[0];
    middle = auto -> text[1:5:2];
    reverse = auto -> text[::-1];
    fn = auto -> lambda(x=auto) -> x * 2;
    result = auto -> fn(3);
}
def sum(first=int -> 0, *rest=int) {
    return code -> first;
}
"#,
    )
    .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    for statement in &main.body.statements[1..4] {
        let Statement::Assignment(assignment) = statement else {
            panic!("expected indexed/sliced assignment");
        };
        let ExprKind::Index { index, .. } = &assignment.value.kind else {
            panic!("expected subscript expression");
        };
        if assignment.name != "char" {
            assert!(matches!(index.kind, ExprKind::Slice { .. }));
        }
    }
    let Statement::Assignment(lambda_assignment) = &main.body.statements[4] else {
        panic!("expected lambda assignment");
    };
    assert!(matches!(
        lambda_assignment.value.kind,
        ExprKind::Lambda { .. }
    ));
    assert_eq!(
        (lambda_assignment.value.line, lambda_assignment.value.column),
        (6, 18)
    );
    let TopLevelItem::Function(sum) = &program.items[1] else {
        panic!("expected sum function");
    };
    assert_eq!(sum.parameters.len(), 2);
    assert_eq!(
        sum.parameters[0].default_value.as_ref().unwrap().kind,
        ExprKind::Number("0".to_owned())
    );
    assert!(sum.parameters[1].variadic);
}

#[test]
fn parses_multiline_comments_and_modulo_expressions() {
    let program =
        parse("/* header\n comment */ int main() {\n # line comment\n value = int -> 5 % 2;\n}\n")
            .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::Assignment(assignment) = &main.body.statements[0] else {
        panic!("expected assignment");
    };
    assert!(matches!(
        assignment.value.kind,
        ExprKind::Binary {
            operator: BinaryOperator::Modulo,
            ..
        }
    ));
    assert_eq!((main.line, main.column), (2, 13));
}

#[test]
fn supports_zero_argument_calls_empty_statements_and_nested_loop_control() {
    let program = parse(
        r#"int main() {
    noop();
    ;
    for item in range() {
        while (True) {
            continue;
            break;
        }
    }
}
"#,
    )
    .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    assert!(matches!(main.body.statements[0], Statement::Expression(_)));
    assert!(matches!(main.body.statements[1], Statement::Empty(_)));
    let Statement::For(for_statement) = &main.body.statements[2] else {
        panic!("expected for loop");
    };
    assert!(matches!(
        for_statement.iterable.kind,
        ExprKind::Call { ref callee, ref arguments } if callee == "range" && arguments.is_empty()
    ));
    let Statement::While(while_statement) = &for_statement.body.statements[0] else {
        panic!("expected nested while loop");
    };
    assert!(matches!(
        while_statement.body.statements[0],
        Statement::Continue(_)
    ));
    assert!(matches!(
        while_statement.body.statements[1],
        Statement::Break(_)
    ));
}

#[test]
fn checks_nested_dictionary_tuple_methods_for_immutability() {
    let program = parse(
        r#"int main() {
    values = auto -> {"pair": (1, 2)};
    values["pair"].clear();
}
"#,
    )
    .unwrap();
    let error = validate_mutability(&program).unwrap_err();
    assert_eq!(error.line, 3);
    assert!(error.message.contains("mutating method `clear`"));
}

#[test]
fn parses_preinstalled_library_imports() {
    let program = parse(
        r#"from library import std;
from library import json;
int main() {}
"#,
    )
    .unwrap();
    let TopLevelItem::Import(std_import) = &program.items[0] else {
        panic!("expected built-in library import");
    };
    assert_eq!(std_import.library, "std");
    let TopLevelItem::Import(json_import) = &program.items[1] else {
        panic!("expected another built-in library import");
    };
    assert_eq!(json_import.library, "json");
}

#[test]
fn parses_gc_control_statements_and_preserves_positions() {
    let program = parse(
        r#"int main() {
    gc! collect;
    gc! pause;
    gc! resume;
}
"#,
    )
    .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let operations: Vec<_> = main
        .body
        .statements
        .iter()
        .map(|statement| {
            let Statement::GcControl(gc) = statement else {
                panic!("expected GC syntax node");
            };
            (gc.operation, gc.line, gc.column)
        })
        .collect();
    assert_eq!(
        operations,
        vec![
            (easyscript::ast::GcOperation::Collect, 2, 5),
            (easyscript::ast::GcOperation::Pause, 3, 5),
            (easyscript::ast::GcOperation::Resume, 4, 5),
        ]
    );
}

#[test]
fn parses_repl_help_and_docstring_nodes() {
    let program = parse(
        r#"int main() {
    help(len);
    docs = auto -> docstring("Calculates a value");
}
"#,
    )
    .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::Expression(help_statement) = &main.body.statements[0] else {
        panic!("expected help expression statement");
    };
    assert!(matches!(
        &help_statement.expression.kind,
        ExprKind::Help { argument }
            if matches!(argument.kind, ExprKind::Identifier(ref name) if name == "len")
    ));
    assert_eq!(
        (
            help_statement.expression.line,
            help_statement.expression.column
        ),
        (2, 5)
    );
    let Statement::Assignment(docs_assignment) = &main.body.statements[1] else {
        panic!("expected docstring assignment");
    };
    assert!(matches!(
        &docs_assignment.value.kind,
        ExprKind::Docstring { value } if value == "Calculates a value"
    ));
    assert_eq!(
        (docs_assignment.value.line, docs_assignment.value.column),
        (3, 20)
    );
}

#[test]
fn parses_thread_declaration_spawn_and_join_as_syntax_only_nodes() {
    let program = parse(
        r#"int main() {
    thread worker {
        println("ready");
    }
    spawn worker;
    join worker;
}
"#,
    )
    .unwrap();
    let TopLevelItem::Function(main) = &program.items[0] else {
        panic!("expected main function");
    };
    let Statement::Thread(thread) = &main.body.statements[0] else {
        panic!("expected thread body node");
    };
    assert_eq!(thread.name, "worker");
    assert_eq!((thread.line, thread.column), (2, 5));
    assert_eq!(thread.body.statements.len(), 1);
    let Statement::Spawn(spawn) = &main.body.statements[1] else {
        panic!("expected spawn node");
    };
    assert_eq!(spawn.name, "worker");
    assert_eq!((spawn.line, spawn.column), (5, 5));
    let Statement::Join(join) = &main.body.statements[2] else {
        panic!("expected join node");
    };
    assert_eq!(join.name, "worker");
    assert_eq!((join.line, join.column), (6, 5));
}

#[test]
fn rejects_unknown_gc_actions_and_invalid_repl_helper_arguments_at_their_locations() {
    let gc_error = parse("int main() {\n    gc! run\n}\n").unwrap_err();
    assert_eq!((gc_error.line, gc_error.column), (2, 9));
    assert!(gc_error.message.contains("unknown GC operation"));

    let help_error = parse("int main() {\n    help()\n}\n").unwrap_err();
    assert_eq!((help_error.line, help_error.column), (2, 5));
    assert!(help_error.message.contains("exactly one expression"));

    let docstring_error = parse("int main() {\n    docstring(value)\n}\n").unwrap_err();
    assert_eq!((docstring_error.line, docstring_error.column), (2, 5));
    assert!(docstring_error.message.contains("string literal"));
}

#[test]
fn parses_structs_and_generic_structs_with_located_typed_fields() {
    let program = parse(
        r#"struct Point {
    x: i32;
    y: i32;
}
struct Pair<T, U> {
    first: T;
    second: Vec<U>;
}
int main() {}
"#,
    )
    .unwrap();
    let TopLevelItem::Struct(point) = &program.items[0] else {
        panic!("expected Point structure");
    };
    assert_eq!(point.name, "Point");
    assert!(point.generic_parameters.is_empty());
    assert_eq!((point.line, point.column), (1, 1));
    assert_eq!(point.fields.len(), 2);
    assert_eq!((point.fields[0].line, point.fields[0].column), (2, 5));
    assert_eq!(point.fields[0].ty.name, "i32");

    let TopLevelItem::Struct(pair) = &program.items[1] else {
        panic!("expected Pair structure");
    };
    assert_eq!(
        pair.generic_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        vec!["T", "U"]
    );
    assert_eq!(pair.fields[0].ty.name, "T");
    assert_eq!(pair.fields[1].ty.name, "Vec");
    assert_eq!(pair.fields[1].ty.generic_arguments[0].name, "U");
}

#[test]
fn parses_unit_tuple_and_named_enum_variants_with_positions() {
    let program = parse(
        r#"enum Color {
    Red,
    Green(u8),
    Blue { r: u8, g: u8, b: u8 }
}
enum Maybe<T> {
    None,
    Some(T),
}
int main() {}
"#,
    )
    .unwrap();
    let TopLevelItem::Enum(color) = &program.items[0] else {
        panic!("expected Color enum");
    };
    assert_eq!(color.name, "Color");
    assert_eq!(color.variants.len(), 3);
    assert_eq!((color.variants[0].line, color.variants[0].column), (2, 5));
    assert!(matches!(
        color.variants[0].payload,
        easyscript::ast::EnumVariantPayload::Unit
    ));
    assert!(matches!(
        &color.variants[1].payload,
        easyscript::ast::EnumVariantPayload::Tuple(types)
            if types.len() == 1 && types[0].name == "u8"
    ));
    assert!(matches!(
        &color.variants[2].payload,
        easyscript::ast::EnumVariantPayload::Struct(fields)
            if fields.iter().map(|field| field.name.as_str()).collect::<Vec<_>>()
                == vec!["r", "g", "b"]
    ));
    let TopLevelItem::Enum(maybe) = &program.items[1] else {
        panic!("expected Maybe enum");
    };
    assert_eq!(maybe.generic_parameters[0].name, "T");
    assert!(matches!(
        &maybe.variants[1].payload,
        easyscript::ast::EnumVariantPayload::Tuple(types)
            if types[0].name == "T"
    ));
}

#[test]
fn rejects_unlisted_fn_syntax_and_keeps_def_declarations() {
    let error = parse("fn map<T, U>(input: T) -> U {}").unwrap_err();
    assert_eq!((error.line, error.column), (1, 1));
    assert!(error.message.contains("only"));

    let program = parse(
        r#"def legacy(value=int) {
    return code -> value;
}
int main() {}
"#,
    )
    .unwrap();
    let TopLevelItem::Function(legacy) = &program.items[0] else {
        panic!("expected def function");
    };
    assert_eq!(legacy.name, "legacy");
    assert_eq!(legacy.parameters[0].ty.name, "int");
    assert!(legacy.generic_parameters.is_empty());
}

#[test]
fn rejects_extern_and_annotation_declarations_outside_the_allowed_grammar() {
    for source in ["extern \"C\" fn c_func() -> int;", "#[aot]\nfn test() {}"] {
        let error = parse(source).unwrap_err();
        assert_eq!((error.line, error.column), (1, 1));
        assert!(error.message.contains("only"));
    }
}

#[test]
fn reports_unlisted_generic_and_attribute_syntax_with_locations() {
    let generic_error = parse("fn map<T,>(input: T) {}").unwrap_err();
    assert_eq!((generic_error.line, generic_error.column), (1, 1));
    assert!(generic_error.message.contains("only"));

    let attribute_error = parse("#[unknown]\nfn test() {}").unwrap_err();
    assert_eq!((attribute_error.line, attribute_error.column), (1, 1));
    assert!(attribute_error.message.contains("only"));
}
