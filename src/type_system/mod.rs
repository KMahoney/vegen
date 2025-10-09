pub mod environment;
pub mod infer;
pub mod solver;
pub mod types;
pub mod uf;

pub use types::Type;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::expr_parser;
    use crate::expr::Expr;
    use crate::ts_type::env_to_ts_type;
    use chumsky::Parser;
    use environment::{Env, InferContext};
    use infer::infer;
    pub use solver::TypeError;
    use solver::{canonical_type, solve};
    use std::collections::HashMap;
    use types::Expected;
    pub use types::{Name, Type};

    fn read_env_types(env: &Env) -> HashMap<Name, Type> {
        let mut out = HashMap::new();
        for (name, ty) in env {
            out.insert(name.clone(), canonical_type(&ty));
        }
        out
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InferResult {
        pub expr_type: Type,
        pub env_types: HashMap<Name, Type>,
        pub env: Env,
    }

    fn infer_types(expr: &Expr) -> Result<InferResult, TypeError> {
        let mut ctx = InferContext::new();
        let mut env = Env::default();
        let mut constraints = Vec::new();
        let ty = infer(
            &mut ctx,
            &mut env,
            &mut constraints,
            expr,
            Expected::_NoExpect,
        );
        solve(&mut ctx, &constraints)?;
        let expr_type = canonical_type(&ty);
        let env_types = read_env_types(&env);
        Ok(InferResult {
            expr_type,
            env_types,
            env,
        })
    }

    fn check(input: &str, expected: &str) {
        let parser = expr_parser(0);
        let expr = parser.parse(input).into_result().unwrap();
        let result = infer_types(&expr).unwrap();
        let ts_type = env_to_ts_type(&result.env);
        assert_eq!(ts_type.to_string(), expected, "for input: {}", input);
    }

    #[test]
    fn variable() {
        check("name", "{ name: any }");
    }

    #[test]
    fn string_literal() {
        check(r#""hello""#, "{}");
    }

    #[test]
    fn string_interpolation() {
        check(r#""hello {name}""#, "{ name: string }");
    }

    #[test]
    fn field_access() {
        check("obj.name", "{ obj: { name: any } }");
    }

    #[test]
    fn nested_field_access() {
        check("obj.inner.value", "{ obj: { inner: { value: any } } }");
    }

    #[test]
    fn function_call() {
        check(
            "fn(a, b)",
            "{ a: any, b: any, fn: (v0: any, v1: any) => any }",
        );
    }

    #[test]
    fn method_call() {
        check(
            "obj.method(arg)",
            "{ arg: any, obj: { method: (v0: any) => any } }",
        );
    }

    #[test]
    fn pipe_simple() {
        check("value | fn", "{ fn: (v0: any) => any, value: any }");
    }

    #[test]
    fn pipe_with_call() {
        check(
            "value | fn(x)",
            "{ fn: (v0: any, v1: any) => any, value: any, x: any }",
        );
    }

    #[test]
    fn chained_pipe() {
        check(
            "a | f1 | f2",
            "{ a: any, f1: (v0: any) => any, f2: (v0: any) => any }",
        );
    }

    #[test]
    fn string_with_field() {
        check(r#""User: {user.name}""#, "{ user: { name: string } }");
    }

    #[test]
    fn multiple_interpolations() {
        check(r#""{first} {last}""#, "{ first: string, last: string }");
    }

    #[test]
    fn call_with_field_args() {
        check(
            "format(user.name, user.age)",
            "{ format: (v0: any, v1: any) => any, user: { age: any, name: any } }",
        );
    }
}
