use itertools::Itertools;
use std::collections::BTreeMap;

use crate::lang::{Expr, Span, StringTemplateSegment};
use crate::type_system::environment::{Env, InferContext};
use crate::type_system::types::{Constraint, Expected, RowDescriptor, Type};

pub fn infer(
    ctx: &mut InferContext,
    env: &mut Env,
    constraints: &mut Vec<Constraint>,
    expr: &Expr,
    expected: Expected,
) -> Type {
    match expr {
        Expr::Variable(name, span) => {
            let ty = env.get(ctx, name);
            expect_equal(span, &ty, &expected, constraints);
            ty
        }
        Expr::Number(_, span) => {
            let ty = Type::Prim("number".to_string());
            expect_equal(span, &ty, &expected, constraints);
            ty
        }
        Expr::StringTemplate(segments, span) => {
            // Infer types for all interpolations and constrain them to string
            for segment in segments {
                if let StringTemplateSegment::Interpolation(interpolated_expr) = segment {
                    infer(
                        ctx,
                        env,
                        constraints,
                        interpolated_expr,
                        Expected::Expect(Type::Prim("string".to_string())),
                    );
                }
            }
            let ty = Type::Prim("string".to_string());
            expect_equal(span, &ty, &expected, constraints);
            ty
        }
        Expr::FunctionCall { callee, args, span } => {
            let fresh_ret_type = Type::Var(ctx.fresh_point());
            let fresh_arg_types = args
                .iter()
                .map(|arg| (arg, Type::Var(ctx.fresh_point())))
                .collect_vec();
            let expected_fn_type = Type::Fun(
                fresh_arg_types.iter().map(|(_, ty)| ty).cloned().collect(),
                Box::new(fresh_ret_type.clone()),
            );

            infer(
                ctx,
                env,
                constraints,
                callee,
                Expected::Expect(expected_fn_type),
            );

            for (arg, ty) in fresh_arg_types {
                infer(ctx, env, constraints, arg, Expected::Expect(ty));
            }

            expect_equal(span, &fresh_ret_type, &expected, constraints);

            fresh_ret_type
        }
        Expr::Field(rec, field_name, span) => {
            let field_point = ctx.fresh_point();
            let tail_point = ctx.fresh_row_point();
            let field_type = Type::Var(field_point.clone());

            let mut fields = BTreeMap::new();
            fields.insert(field_name.clone(), field_type.clone());

            let row_point = {
                let id = tail_point.id();
                crate::type_system::uf::fresh(id, RowDescriptor::RowExtend(fields, tail_point))
            };

            let wanted_row = Type::Record(row_point);
            infer(ctx, env, constraints, rec, Expected::Expect(wanted_row));

            expect_equal(span, &field_type, &expected, constraints);

            field_type
        }
        Expr::Pipe { left, right, span } => {
            // Desugar pipe based on right side structure
            match right.as_ref() {
                // If right is a function call, prepend left as first argument
                Expr::FunctionCall { callee, args, .. } => {
                    let mut all_args = Vec::with_capacity(args.len() + 1);
                    all_args.push(left.as_ref().clone());
                    all_args.extend(args.iter().cloned());

                    let desugared = Expr::FunctionCall {
                        callee: callee.clone(),
                        args: all_args,
                        span: *span,
                    };

                    infer(ctx, env, constraints, &desugared, expected)
                }
                // Otherwise, treat right as a function and apply left as its argument
                _ => {
                    let desugared = Expr::FunctionCall {
                        callee: Box::new(right.as_ref().clone()),
                        args: vec![left.as_ref().clone()],
                        span: *span,
                    };

                    infer(ctx, env, constraints, &desugared, expected)
                }
            }
        }
    }
}

fn expect_equal(
    span: &Span,
    actual: &Type,
    expected: &Expected,
    constraints: &mut Vec<Constraint>,
) {
    match expected {
        Expected::Expect(target) => {
            constraints.push(Constraint::Equal(*span, actual.clone(), target.clone()));
        }
        Expected::NoExpect => {}
    }
}
