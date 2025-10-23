pub mod environment;
pub mod infer;
pub mod solver;
pub mod types;
pub mod uf;

pub use types::Type;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::expr_parser;
    use crate::lang::Expr;
    use crate::ts_type::env_to_ts_type;
    use chumsky::Parser;
    use environment::{Env, InferContext};
    use infer::infer;
    pub use solver::TypeError;
    use solver::{canonical_type, solve};
    use std::collections::{BTreeMap, HashMap};
    use types::{Descriptor, Expected, Name, RowDescriptor, Type};

    fn read_env_types(env: &Env) -> HashMap<Name, Type> {
        let mut out = HashMap::new();
        for (name, ty) in env {
            out.insert(name.clone(), canonical_type(ty));
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
            Expected::NoExpect,
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

    #[test]
    fn instantiate_unbound_var() {
        let mut ctx = InferContext::new();
        let var = Type::Var(ctx.fresh_point());
        let instantiated = ctx.instantiate(&var);

        match instantiated {
            Type::Var(p) => {
                // Fresh point, different from original
                assert_ne!(
                    p.id(),
                    match &var {
                        Type::Var(orig) => orig.id(),
                        _ => unreachable!(),
                    }
                );
            }
            _ => panic!("Expected Var"),
        }
    }

    #[test]
    fn instantiate_bound_var() {
        let mut ctx = InferContext::new();
        let bound_type = Type::Prim("number".to_string());
        let bound_point = ctx.fresh_descriptor(Descriptor::Bound(Box::new(bound_type.clone())));
        let var = Type::Var(bound_point);
        let instantiated = ctx.instantiate(&var);

        assert_eq!(instantiated, bound_type);
    }

    #[test]
    fn instantiate_record() {
        let mut ctx = InferContext::new();
        let record = Type::Record(ctx.fresh_row_point());
        let instantiated = ctx.instantiate(&record);

        match instantiated {
            Type::Record(p) => {
                // Fresh point
                assert_ne!(
                    p.id(),
                    match &record {
                        Type::Record(orig) => orig.id(),
                        _ => unreachable!(),
                    }
                );
            }
            _ => panic!("Expected Record"),
        }
    }

    #[test]
    fn instantiate_function() {
        let mut ctx = InferContext::new();
        let fun = Type::Fun(
            vec![
                Type::Var(ctx.fresh_point()),
                Type::Prim("string".to_string()),
            ],
            Box::new(Type::Var(ctx.fresh_point())),
        );
        let instantiated = ctx.instantiate(&fun);

        match instantiated {
            Type::Fun(args, ret) => {
                // First arg fresh, second not
                match &args[0] {
                    Type::Var(p) => assert_ne!(
                        p.id(),
                        match &fun {
                            Type::Fun(orig, _) => match &orig[0] {
                                Type::Var(op) => op.id(),
                                _ => unreachable!(),
                            },
                            _ => unreachable!(),
                        }
                    ),
                    _ => panic!("Expected first arg Var"),
                }
                let orig_args = match &fun {
                    Type::Fun(orig_args, _) => orig_args,
                    _ => unreachable!(),
                };
                assert_eq!(args[1], orig_args[1]);
                match *ret {
                    Type::Var(p) => assert_ne!(
                        p.id(),
                        match &fun {
                            Type::Fun(_, orig_ret) => match &**orig_ret {
                                Type::Var(op) => op.id(),
                                _ => unreachable!(),
                            },
                            _ => unreachable!(),
                        }
                    ),
                    _ => panic!("Expected ret Var"),
                }
            }
            _ => panic!("Expected Fun"),
        }
    }

    #[test]
    fn instantiate_sharing_same_var() {
        let mut ctx = InferContext::new();
        let shared_point = ctx.fresh_point();
        let fun = Type::Fun(
            vec![Type::Var(shared_point.clone())],
            Box::new(Type::Var(shared_point)),
        );
        let instantiated = ctx.instantiate(&fun);

        match instantiated {
            Type::Fun(args, ret) => {
                // Both should be the same fresh var
                match (&args[0], &*ret) {
                    (Type::Var(p1), Type::Var(p2)) => assert_eq!(p1, p2),
                    _ => panic!("Expected same Var both places"),
                }
            }
            _ => panic!("Expected Fun"),
        }
    }

    #[test]
    fn instantiate_discriminated_union() {
        let mut ctx = InferContext::new();
        let mut branches = BTreeMap::new();
        branches.insert("a".to_string(), ctx.fresh_row_point());
        branches.insert("b".to_string(), ctx.fresh_row_point());
        let du = Type::DiscriminatedUnion(branches);
        let instantiated = ctx.instantiate(&du);

        match instantiated {
            Type::DiscriminatedUnion(new_branches) => {
                assert_eq!(new_branches.len(), 2);
                let orig_branches = match &du {
                    Type::DiscriminatedUnion(b) => b,
                    _ => unreachable!(),
                };
                for (k, p) in new_branches {
                    let orig_p = orig_branches.get(&k).unwrap();
                    assert_ne!(p.id(), orig_p.id());
                }
            }
            _ => panic!("Expected DiscriminatedUnion"),
        }
    }

    #[test]
    fn instantiate_extended_record() {
        use crate::type_system::uf::get;

        let mut ctx = InferContext::new();
        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), Type::Prim("string".to_string()));
        fields.insert("age".to_string(), Type::Var(ctx.fresh_point()));
        let ext_point = ctx.fresh_row_point();
        let record_point = ctx.fresh_row_extend(fields, ext_point.clone());
        let record = Type::Record(record_point.clone());
        let instantiated = ctx.instantiate(&record);

        match instantiated {
            Type::Record(new_point) => {
                // Original point should be different
                assert_ne!(new_point.id(), record_point.id());

                // Check the RowDescriptor
                if let RowDescriptor::RowExtend(new_fields, new_rest_point) = get(&new_point) {
                    assert_eq!(new_fields.len(), 2);
                    assert_eq!(
                        new_fields.get("name"),
                        Some(&Type::Prim("string".to_string()))
                    );

                    // The age field should be a fresh Var
                    if let Some(Type::Var(age_var)) = new_fields.get("age") {
                        // Get the original age var
                        if let RowDescriptor::RowExtend(orig_fields, _) = get(&record_point) {
                            if let Type::Var(orig_age_var) = orig_fields.get("age").unwrap() {
                                assert_ne!(age_var.id(), orig_age_var.id());
                            }
                        }
                    } else {
                        panic!("Expected 'age' to be Var");
                    }

                    // The rest point should be fresh
                    assert_ne!(new_rest_point.id(), ext_point.id());
                } else {
                    panic!("Expected RowExtend");
                }
            }
            _ => panic!("Expected Record"),
        }
    }

    #[test]
    fn instantiate_chained_extended_record() {
        use crate::type_system::uf::get;

        let mut ctx = InferContext::new();

        // Create a chained row: { outer_field: string, inner_field: bool, ...base_rest }
        let mut inner_fields = BTreeMap::new();
        inner_fields.insert("inner_field".to_string(), Type::Prim("bool".to_string()));
        let base_rest = ctx.fresh_row_point();

        let mut outer_fields = BTreeMap::new();
        outer_fields.insert("outer_field".to_string(), Type::Prim("string".to_string()));
        let var_in_outer = ctx.fresh_point();
        outer_fields.insert("var_field".to_string(), Type::Var(var_in_outer.clone()));

        let inner_extend = ctx.fresh_row_extend(inner_fields, base_rest.clone());
        let outer_point = ctx.fresh_row_extend(outer_fields, inner_extend.clone());

        let record = Type::Record(outer_point);
        let instantiated = ctx.instantiate(&record);

        match instantiated {
            Type::Record(new_outer_point) => {
                // Check outer RowExtend
                if let RowDescriptor::RowExtend(new_outer_fields, outer_rest_point) =
                    get(&new_outer_point)
                {
                    assert_eq!(new_outer_fields.len(), 3);
                    assert_eq!(
                        new_outer_fields.get("outer_field"),
                        Some(&Type::Prim("string".to_string()))
                    );
                    assert_eq!(
                        new_outer_fields.get("inner_field"),
                        Some(&Type::Prim("bool".to_string()))
                    );

                    // Var field should be fresh
                    if let Some(Type::Var(new_var_field)) = new_outer_fields.get("var_field") {
                        assert_ne!(new_var_field.id(), var_in_outer.id());
                    } else {
                        panic!("Expected var_field to be Var");
                    }

                    // The rest should be a fresh RowFlex, as instantiation cuts the chain and makes rest polymorphic
                    if let RowDescriptor::RowFlex(_) = get(&outer_rest_point) {
                        // Fresh point, different from inner_extend
                        assert_ne!(outer_rest_point.id(), inner_extend.id());
                        assert_ne!(outer_rest_point.id(), base_rest.id());
                    } else {
                        panic!("Expected RowFlex for outer rest");
                    }
                } else {
                    panic!("Expected RowExtend for outer");
                }
            }
            _ => panic!("Expected Record"),
        }
    }
}
