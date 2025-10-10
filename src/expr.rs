use std::collections::HashSet;

use chumsky::prelude::*;
use chumsky::span::SimpleSpan;
use itertools::Itertools;

use crate::builtins::BUILTINS;

pub type SourceId = usize;
pub type Span = SimpleSpan<usize, SourceId>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    StringTemplate(Vec<StringTemplateSegment>, Span),
    Variable(String, Span),
    Number(String, Span),
    Field(Box<Expr>, String, Span),
    FunctionCall {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Pipe {
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> &Span {
        match self {
            Expr::StringTemplate(_, span) => span,
            Expr::Variable(_, span) => span,
            Expr::Number(_, span) => span,
            Expr::Field(_, _, span) => span,
            Expr::FunctionCall { span, .. } => span,
            Expr::Pipe { span, .. } => span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringTemplateSegment {
    Literal(String),
    Interpolation(Box<Expr>),
}

// Helper enum for postfix operations
#[derive(Debug, Clone)]
enum PostfixOp {
    Field(String),
    Call(Vec<Expr>),
}

pub fn expr_parser<'a>(
    source: SourceId,
) -> impl Parser<'a, &'a str, Expr, extra::Err<Rich<'a, char>>> {
    recursive(move |expr| {
        // Helper to create a span with source context
        fn sourced_span(source: SourceId, span: SimpleSpan) -> Span {
            SimpleSpan {
                start: span.start,
                end: span.end,
                context: source,
            }
        }

        // Basic identifier parser
        let identifier = any()
            .filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
            .then(
                any()
                    .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .repeated()
                    .collect::<Vec<char>>(),
            )
            .map(|(first, rest)| {
                let mut result = String::new();
                result.push(first);
                result.extend(rest);
                result
            })
            .padded()
            .labelled("identifier")
            .boxed();

        // String template parser
        let string_template = just('"')
            .ignore_then(
                choice((
                    // Literal segment: any text except { and "
                    none_of("{\"")
                        .repeated()
                        .at_least(1)
                        .to_slice()
                        .map(|s: &str| StringTemplateSegment::Literal(s.to_string())),
                    // Interpolation segment: {expr}
                    just('{')
                        .ignore_then(expr.clone())
                        .then_ignore(just('}'))
                        .map(|e| StringTemplateSegment::Interpolation(Box::new(e))),
                ))
                .repeated()
                .collect(),
            )
            .then_ignore(just('"'))
            .map_with(move |segments, e| {
                Expr::StringTemplate(segments, sourced_span(source, e.span()))
            })
            .labelled("string template")
            .boxed();

        // Number parser
        let number = text::int(10)
            .then(
                just('.')
                    .then(text::digits(10).collect::<String>())
                    .or_not(),
            )
            .map(|(int_part, decimal): (&str, Option<(char, String)>)| {
                let mut n = int_part.to_string();
                if let Some((dot, frac)) = decimal {
                    n.push(dot);
                    n.push_str(&frac);
                }
                n
            })
            .padded()
            .labelled("number")
            .boxed();

        // Atomic expressions (no postfix operations)
        let atom = choice((
            number.map_with(move |n, e| Expr::Number(n, sourced_span(source, e.span()))),
            string_template,
            identifier
                .clone()
                .map_with(move |name, e| Expr::Variable(name, sourced_span(source, e.span()))),
            just('(').ignore_then(expr.clone()).then_ignore(just(')')),
        ))
        .boxed();

        // Postfix operations: field access and function calls
        // These can be chained: obj.field.method(arg).anotherField()
        let postfix = atom
            .then(
                choice((
                    // Field access: .identifier
                    just('.')
                        .ignore_then(identifier.clone())
                        .map(PostfixOp::Field),
                    // Function call: (args)
                    just('(')
                        .ignore_then(
                            expr.clone()
                                .padded()
                                .separated_by(just(','))
                                .allow_trailing()
                                .collect::<Vec<Expr>>(),
                        )
                        .then_ignore(just(')'))
                        .map(PostfixOp::Call),
                ))
                .repeated()
                .collect::<Vec<PostfixOp>>(),
            )
            .map_with(move |(base, ops), e| {
                let mut current = base;
                for op in ops {
                    match op {
                        PostfixOp::Field(field) => {
                            current = Expr::Field(
                                Box::new(current),
                                field,
                                sourced_span(source, e.span()),
                            );
                        }
                        PostfixOp::Call(args) => {
                            current = Expr::FunctionCall {
                                callee: Box::new(current),
                                args,
                                span: sourced_span(source, e.span()),
                            };
                        }
                    }
                }
                current
            })
            .labelled("expression with postfix operations")
            .boxed();

        let primary = postfix;

        // Pipe expression parser (left associative)
        let pipe_expr = primary
            .separated_by(just('|').padded())
            .at_least(1)
            .collect::<Vec<Expr>>()
            .map(move |exprs| {
                if exprs.len() == 1 {
                    exprs.into_iter().next().unwrap()
                } else {
                    let mut current = exprs[0].clone();
                    for right in &exprs[1..] {
                        // Compute span from left's start to right's end
                        let span = SimpleSpan {
                            start: current.span().start,
                            end: right.span().end,
                            context: source,
                        };
                        current = Expr::Pipe {
                            left: Box::new(current),
                            right: Box::new(right.clone()),
                            span,
                        };
                    }
                    current
                }
            })
            .boxed();

        pipe_expr
    })
    .padded()
    .boxed()
}

pub fn expr_dependencies(expr: &Expr) -> HashSet<String> {
    fn collect_path(expr: &Expr, deps: &mut HashSet<String>) {
        let mut current_path = Vec::new();
        let mut node = expr;

        loop {
            match node {
                Expr::Variable(name, _) => {
                    current_path.push(name.clone());
                    let dep = current_path.into_iter().rev().join(".");
                    if !BUILTINS.contains_key(&dep) {
                        deps.insert(dep);
                    }
                    break;
                }
                Expr::Number(_, _) => {
                    break;
                }
                Expr::Field(base, field, _) => {
                    current_path.push(field.clone());
                    node = base;
                }
                Expr::FunctionCall { callee, args, .. } => {
                    current_path.clear();
                    node = callee;
                    for arg in args {
                        collect_path(arg, deps);
                    }
                }
                Expr::Pipe { left, right, .. } => {
                    current_path.clear();
                    collect_path(left, deps);
                    node = right;
                }
                Expr::StringTemplate(segments, _) => {
                    for segment in segments {
                        if let StringTemplateSegment::Interpolation(e) = segment {
                            collect_path(e, deps);
                        }
                    }
                    break;
                }
            }
        }
    }

    let mut deps = HashSet::new();
    collect_path(expr, &mut deps);
    deps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestExpr {
        StringTemplate(Vec<TestStringTemplateSegment>),
        Variable(&'static str),
        Number(&'static str),
        Field(Box<TestExpr>, &'static str),
        FunctionCall {
            callee: Box<TestExpr>,
            args: Vec<TestExpr>,
        },
        Pipe {
            left: Box<TestExpr>,
            right: Box<TestExpr>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestStringTemplateSegment {
        Literal(&'static str),
        Interpolation(Box<TestExpr>),
    }

    fn assert_expr_matches(expr: &Expr, expected: &TestExpr) {
        match (expr, expected) {
            (Expr::Variable(name, _), TestExpr::Variable(test_name)) => {
                assert_eq!(name, test_name);
            }
            (Expr::Number(n, _), TestExpr::Number(test_n)) => {
                assert_eq!(n, test_n);
            }
            (Expr::Field(base, field, _), TestExpr::Field(test_base, test_field)) => {
                assert_expr_matches(base, test_base);
                assert_eq!(field, test_field);
            }
            (
                Expr::FunctionCall { callee, args, .. },
                TestExpr::FunctionCall {
                    callee: test_callee,
                    args: test_args,
                },
            ) => {
                assert_expr_matches(callee, test_callee);
                assert_eq!(args.len(), test_args.len());
                for (arg, test_arg) in args.iter().zip(test_args) {
                    assert_expr_matches(arg, test_arg);
                }
            }
            (Expr::StringTemplate(segments, _), TestExpr::StringTemplate(test_segments)) => {
                assert_eq!(segments.len(), test_segments.len());
                for (seg, test_seg) in segments.iter().zip(test_segments) {
                    match (seg, test_seg) {
                        (
                            StringTemplateSegment::Literal(s),
                            TestStringTemplateSegment::Literal(test_s),
                        ) => assert_eq!(s, test_s),
                        (
                            StringTemplateSegment::Interpolation(e),
                            TestStringTemplateSegment::Interpolation(test_e),
                        ) => assert_expr_matches(e, test_e),
                        _ => panic!("Segment mismatch"),
                    }
                }
            }
            (
                Expr::Pipe { left, right, .. },
                TestExpr::Pipe {
                    left: test_left,
                    right: test_right,
                },
            ) => {
                assert_expr_matches(left, test_left);
                assert_expr_matches(right, test_right);
            }
            _ => panic!("Expr variant mismatch"),
        }
    }

    type ParseError = Error;
    fn parse_expr(input: &str, source: SourceId) -> Result<Expr, Vec<ParseError>> {
        let parser = expr_parser(source);

        match parser.parse(input).into_result() {
            Ok(expr) => Ok(expr),
            Err(errors) => Err(errors
                .into_iter()
                .map(|error| {
                    let span = Span::new(source, error.span().start..error.span().end);
                    ParseError {
                        message: format!("{}", error.reason()),
                        main_span: span,
                        labels: vec![(span, "Parse error here".to_string())],
                    }
                })
                .collect()),
        }
    }

    fn parse(input: &str) -> Result<Expr, Vec<ParseError>> {
        parse_expr(input, 0)
    }

    #[test]
    fn test_simple() {
        let expr = parse("a").unwrap();
        let expected = TestExpr::Variable("a");
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_variable_path() {
        let expr = parse("a.b.c").unwrap();
        let expected = TestExpr::Field(
            Box::new(TestExpr::Field(Box::new(TestExpr::Variable("a")), "b")),
            "c",
        );
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_simple_variable() {
        let expr = parse("foo").unwrap();
        let expected = TestExpr::Variable("foo");
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_function_call_no_args() {
        let expr = parse("fn()").unwrap();
        let expected = TestExpr::FunctionCall {
            callee: Box::new(TestExpr::Variable("fn")),
            args: vec![],
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_function_call_with_args() {
        let expr = parse("add(a, b)").unwrap();
        let expected = TestExpr::FunctionCall {
            callee: Box::new(TestExpr::Variable("add")),
            args: vec![TestExpr::Variable("a"), TestExpr::Variable("b")],
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_function_call_with_string_args() {
        let expr = parse("add(\"a\", \"b\")").unwrap();
        let expected = TestExpr::FunctionCall {
            callee: Box::new(TestExpr::Variable("add")),
            args: vec![
                TestExpr::StringTemplate(vec![TestStringTemplateSegment::Literal("a")]),
                TestExpr::StringTemplate(vec![TestStringTemplateSegment::Literal("b")]),
            ],
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_string_template_literal_only() {
        let expr = parse("\"hello world\"").unwrap();
        let expected =
            TestExpr::StringTemplate(vec![TestStringTemplateSegment::Literal("hello world")]);
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_string_template_with_interpolation() {
        let expr = parse("\"hello {name}\"").unwrap();
        let expected = TestExpr::StringTemplate(vec![
            TestStringTemplateSegment::Literal("hello "),
            TestStringTemplateSegment::Interpolation(Box::new(TestExpr::Variable("name"))),
        ]);
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_pipe_expression() {
        let expr = parse("a | fn(b)").unwrap();
        let expected = TestExpr::Pipe {
            left: Box::new(TestExpr::Variable("a")),
            right: Box::new(TestExpr::FunctionCall {
                callee: Box::new(TestExpr::Variable("fn")),
                args: vec![TestExpr::Variable("b")],
            }),
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_chained_pipe() {
        let expr = parse("a | f1(b) | f2(c)").unwrap();
        let expected = TestExpr::Pipe {
            left: Box::new(TestExpr::Pipe {
                left: Box::new(TestExpr::Variable("a")),
                right: Box::new(TestExpr::FunctionCall {
                    callee: Box::new(TestExpr::Variable("f1")),
                    args: vec![TestExpr::Variable("b")],
                }),
            }),
            right: Box::new(TestExpr::FunctionCall {
                callee: Box::new(TestExpr::Variable("f2")),
                args: vec![TestExpr::Variable("c")],
            }),
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_nested_function_call() {
        let expr = parse("outer(inner(a))").unwrap();
        let expected = TestExpr::FunctionCall {
            callee: Box::new(TestExpr::Variable("outer")),
            args: vec![TestExpr::FunctionCall {
                callee: Box::new(TestExpr::Variable("inner")),
                args: vec![TestExpr::Variable("a")],
            }],
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_parenthesized_expression() {
        let expr = parse("(a)").unwrap();
        let expected = TestExpr::Variable("a");
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_complex_expression() {
        let expr = parse("\"User: {user.name | format(\"{first} {last}\")}\"").unwrap();
        let expected = TestExpr::StringTemplate(vec![
            TestStringTemplateSegment::Literal("User: "),
            TestStringTemplateSegment::Interpolation(Box::new(TestExpr::Pipe {
                left: Box::new(TestExpr::Field(
                    Box::new(TestExpr::Variable("user")),
                    "name",
                )),
                right: Box::new(TestExpr::FunctionCall {
                    callee: Box::new(TestExpr::Variable("format")),
                    args: vec![TestExpr::StringTemplate(vec![
                        TestStringTemplateSegment::Interpolation(Box::new(TestExpr::Variable(
                            "first",
                        ))),
                        TestStringTemplateSegment::Literal(" "),
                        TestStringTemplateSegment::Interpolation(Box::new(TestExpr::Variable(
                            "last",
                        ))),
                    ])],
                }),
            })),
        ]);
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_number_integer() {
        let expr = parse("42").unwrap();
        let expected = TestExpr::Number("42");
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_number_float() {
        let expr = parse("3.14").unwrap();
        let expected = TestExpr::Number("3.14");
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_function_call_with_number_args() {
        let expr = parse("add(42, 3.14)").unwrap();
        let expected = TestExpr::FunctionCall {
            callee: Box::new(TestExpr::Variable("add")),
            args: vec![TestExpr::Number("42"), TestExpr::Number("3.14")],
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_parse_error() {
        assert!(parse("invalid{").is_err());
    }

    #[test]
    fn test_method_call() {
        let expr = parse("obj.method()").unwrap();
        let expected = TestExpr::FunctionCall {
            callee: Box::new(TestExpr::Field(
                Box::new(TestExpr::Variable("obj")),
                "method",
            )),
            args: vec![],
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_method_call_with_args() {
        let expr = parse("obj.method(a, b)").unwrap();
        let expected = TestExpr::FunctionCall {
            callee: Box::new(TestExpr::Field(
                Box::new(TestExpr::Variable("obj")),
                "method",
            )),
            args: vec![TestExpr::Variable("a"), TestExpr::Variable("b")],
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_chained_method_calls() {
        let expr = parse("obj.method1().method2()").unwrap();
        let expected = TestExpr::FunctionCall {
            callee: Box::new(TestExpr::Field(
                Box::new(TestExpr::FunctionCall {
                    callee: Box::new(TestExpr::Field(
                        Box::new(TestExpr::Variable("obj")),
                        "method1",
                    )),
                    args: vec![],
                }),
                "method2",
            )),
            args: vec![],
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_call_parenthesized_expression() {
        let expr = parse("(getFunc())()").unwrap();
        let expected = TestExpr::FunctionCall {
            callee: Box::new(TestExpr::FunctionCall {
                callee: Box::new(TestExpr::Variable("getFunc")),
                args: vec![],
            }),
            args: vec![],
        };
        assert_expr_matches(&expr, &expected);
    }

    #[test]
    fn test_call_with_arg_then_method() {
        let expr = parse("getFunc(arg).method()").unwrap();
        let expected = TestExpr::FunctionCall {
            callee: Box::new(TestExpr::Field(
                Box::new(TestExpr::FunctionCall {
                    callee: Box::new(TestExpr::Variable("getFunc")),
                    args: vec![TestExpr::Variable("arg")],
                }),
                "method",
            )),
            args: vec![],
        };
        assert_expr_matches(&expr, &expected);
    }

    // Tests for expr_dependencies

    #[test]
    fn test_dependencies_simple_variable() {
        let expr = parse("a").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(deps, HashSet::from(["a".to_string()]));
    }

    #[test]
    fn test_dependencies_field_access() {
        let expr = parse("a.b.c").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(deps, HashSet::from(["a.b.c".to_string()]));
    }

    #[test]
    fn test_dependencies_function_call_with_field_args() {
        // f(a.x, a.y) should have dependencies: f, a.x, a.y
        let expr = parse("f(a.x, a.y)").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(
            deps,
            HashSet::from(["f".to_string(), "a.x".to_string(), "a.y".to_string()])
        );
    }

    #[test]
    fn test_dependencies_path_before_call() {
        // a.b.c().d.e should have dependency: a.b.c
        // The path before the first call operation is the dependency
        let expr = parse("a.b.c().d.e").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(deps, HashSet::from(["a.b.c".to_string()]));
    }

    #[test]
    fn test_dependencies_multiple_calls_in_chain() {
        // a.b().c().d should have dependency: a.b (path before first call)
        let expr = parse("a.b().c().d").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(deps, HashSet::from(["a.b".to_string()]));
    }

    #[test]
    fn test_dependencies_nested_calls_with_args() {
        // outer(inner(a.b), c.d) should have: outer, inner, a.b, c.d
        let expr = parse("outer(inner(a.b), c.d)").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(
            deps,
            HashSet::from([
                "outer".to_string(),
                "inner".to_string(),
                "a.b".to_string(),
                "c.d".to_string()
            ])
        );
    }

    #[test]
    fn test_dependencies_method_call() {
        // obj.method(arg) should have: obj.method, arg
        let expr = parse("obj.method(arg)").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(
            deps,
            HashSet::from(["obj.method".to_string(), "arg".to_string()])
        );
    }

    #[test]
    fn test_dependencies_chained_method_call_arg() {
        // obj.method(arg) should have: obj.method, arg
        let expr = parse("obj.method(arg).extra").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(
            deps,
            HashSet::from(["obj.method".to_string(), "arg".to_string()])
        );
    }

    #[test]
    fn test_dependencies_chained_method_calls() {
        // obj.method1().method2() should have: obj.method1
        let expr = parse("obj.method1().method2()").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(deps, HashSet::from(["obj.method1".to_string()]));
    }

    #[test]
    fn test_dependencies_in_string_template() {
        // "hello {user.name}" should have: user.name
        let expr = parse("\"hello {user.name}\"").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(deps, HashSet::from(["user.name".to_string()]));
    }

    #[test]
    fn test_dependencies_in_pipe() {
        // a.b | f(c.d) should have: a.b, f, c.d
        let expr = parse("a.b | f(c.d)").unwrap();
        let deps = expr_dependencies(&expr);
        assert_eq!(
            deps,
            HashSet::from(["a.b".to_string(), "f".to_string(), "c.d".to_string()])
        );
    }
}
