use std::collections::BTreeMap;

use crate::ast::Span;
use crate::error::Error as VegenError;
use crate::ts_type::type_to_ts_type;
use crate::type_system::environment::InferContext;
use crate::type_system::types::{Constraint, Descriptor, FlexMark, Name, RowDescriptor, Type};
use crate::type_system::uf::{get, set, union, Point};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeError {
    PrimMismatch {
        span: Span,
        expected: String,
        actual: String,
    },
    ArityMismatch {
        span: Span,
        expected: usize,
        actual: usize,
    },
    StructMismatch {
        span: Span,
        expected: Type,
        actual: Type,
    },
    OccursCheck {
        span: Span,
        ty: Type,
    },
    RowMismatch {
        span: Span,
        message: String,
    },
    UnionKeyMismatch {
        span: Span,
        expected: Vec<String>,
        actual: Vec<String>,
    },
}

impl TypeError {
    pub fn to_error(&self) -> VegenError {
        match self {
            TypeError::PrimMismatch {
                span,
                expected,
                actual,
            } => {
                let labels = vec![(*span, format!("This has type {}", actual))];
                let message = format!("Type mismatch: expected {}, got {}", expected, actual);

                VegenError {
                    message,
                    main_span: *span,
                    labels,
                }
            }
            TypeError::ArityMismatch {
                span,
                expected,
                actual,
            } => VegenError {
                message: format!(
                    "Function arity mismatch: expected {} arguments, got {}",
                    expected, actual
                ),
                main_span: *span,
                labels: vec![(*span, format!("Expected {} arguments", expected))],
            },
            TypeError::StructMismatch {
                span,
                expected,
                actual,
            } => {
                let expected = type_to_ts_type(expected).to_string();
                let actual = type_to_ts_type(actual).to_string();
                let labels = vec![(*span, format!("This has type {}", actual))];
                let message = format!(
                    "Type structure mismatch: expected {}, got {}",
                    expected, actual
                );
                VegenError {
                    message,
                    main_span: *span,
                    labels,
                }
            }
            TypeError::OccursCheck { span, ty } => VegenError {
                message: format!("Infinite type detected: {}", ty),
                main_span: *span,
                labels: vec![(*span, "This creates an infinite type".to_string())],
            },
            TypeError::RowMismatch { span, message } => VegenError {
                message: format!("Record type error: {}", message),
                main_span: *span,
                labels: vec![(*span, message.clone())],
            },
            TypeError::UnionKeyMismatch {
                span,
                expected,
                actual,
            } => {
                let labels = vec![(*span, "Union variants do not align".to_string())];
                let message = format!(
                    "Discriminated union key mismatch: expected {{{}}}, got {{{}}}",
                    expected.join(", "),
                    actual.join(", ")
                );
                VegenError {
                    message,
                    main_span: *span,
                    labels,
                }
            }
        }
    }
}

pub fn solve(ctx: &mut InferContext, constraints: &Vec<Constraint>) -> Result<(), TypeError> {
    for constraint in constraints {
        match constraint {
            Constraint::Equal(span, t1, t2) => unify(ctx, span, t1, t2)?,
        }
    }
    Ok(())
}

pub fn canonical_type(ty: &Type) -> Type {
    match ty {
        Type::Prim(_) => ty.clone(),
        Type::Fun(args, res) => {
            let args = args.iter().map(canonical_type).collect();
            let res = Box::new(canonical_type(res));
            Type::Fun(args, res)
        }
        Type::Array(elem) => {
            let elem = Box::new(canonical_type(elem));
            Type::Array(elem)
        }
        Type::Var(point) => match get(point) {
            Descriptor::Bound(bound) => canonical_type(&bound),
            Descriptor::Unbound(_) => Type::Var(point.clone()),
        },
        Type::Record(row_point) => {
            let row = canonical_row_point(row_point);
            Type::Record(row)
        }
        Type::DiscriminatedUnion(map) => {
            let mut new_map = BTreeMap::new();
            for (k, rp) in map {
                let row = canonical_row_point(rp);
                new_map.insert(k.clone(), row);
            }
            Type::DiscriminatedUnion(new_map)
        }
    }
}

fn canonical_row_point(row_point: &Point<RowDescriptor>) -> Point<RowDescriptor> {
    let desc = get_row(row_point);
    match desc {
        RowDescriptor::RowFlex(_) => row_point.clone(),
        RowDescriptor::RowExtend(fields, tail) => {
            let mut canonical_fields = BTreeMap::new();
            for (name, ty) in fields {
                canonical_fields.insert(name.clone(), canonical_type(&ty));
            }
            let canonical_tail = canonical_row_point(&tail);
            let id = row_point.id();
            crate::type_system::uf::fresh(
                id,
                RowDescriptor::RowExtend(canonical_fields, canonical_tail),
            )
        }
    }
}

fn get_row(point: &Point<RowDescriptor>) -> RowDescriptor {
    get(point)
}

fn unify(ctx: &mut InferContext, span: &Span, t1: &Type, t2: &Type) -> Result<(), TypeError> {
    let t1 = canonical_type(t1);
    let t2 = canonical_type(t2);
    match (t1, t2) {
        (Type::Var(p1), Type::Var(p2)) => unify_points(ctx, span, &p1, &p2),
        (Type::Var(point), ty) | (ty, Type::Var(point)) => bind_variable(span, &point, &ty),
        (Type::Fun(args1, res1), Type::Fun(args2, res2)) => {
            if args1.len() != args2.len() {
                return Err(TypeError::ArityMismatch {
                    span: *span,
                    expected: args2.len(),
                    actual: args1.len(),
                });
            }
            for (a1, a2) in args1.iter().zip(args2.iter()) {
                unify(ctx, span, a1, a2)?;
            }
            unify(ctx, span, &res1, &res2)
        }
        (Type::Array(e1), Type::Array(e2)) => unify(ctx, span, &e1, &e2),
        (Type::Prim(p1), Type::Prim(p2)) => {
            if p1 == p2 {
                Ok(())
            } else {
                Err(TypeError::PrimMismatch {
                    span: *span,
                    expected: p2,
                    actual: p1,
                })
            }
        }
        (Type::Record(r1), Type::Record(r2)) => unify_rows(ctx, span, &r1, &r2),
        (Type::DiscriminatedUnion(m1), Type::DiscriminatedUnion(m2)) => {
            let keys1: Vec<String> = m1.keys().cloned().collect();
            let keys2: Vec<String> = m2.keys().cloned().collect();
            if keys1 != keys2 {
                return Err(TypeError::UnionKeyMismatch {
                    span: *span,
                    expected: keys2,
                    actual: keys1,
                });
            }
            for (k, rp1) in m1.iter() {
                let rp2 = m2.get(k).unwrap();
                unify_rows(ctx, span, rp1, rp2)?;
            }
            Ok(())
        }
        (left, right) => Err(TypeError::StructMismatch {
            span: *span,
            expected: right,
            actual: left,
        }),
    }
}

fn unify_points(
    ctx: &mut InferContext,
    span: &Span,
    p1: &Point<Descriptor>,
    p2: &Point<Descriptor>,
) -> Result<(), TypeError> {
    if p1 == p2 {
        return Ok(());
    }

    let desc1 = get(p1);
    let desc2 = get(p2);

    match (desc1.clone(), desc2.clone()) {
        (Descriptor::Bound(bound), _) => unify(ctx, span, &bound, &Type::Var(p2.clone())),
        (_, Descriptor::Bound(bound)) => unify(ctx, span, &Type::Var(p1.clone()), &bound),
        (Descriptor::Unbound(mark1), Descriptor::Unbound(mark2)) => {
            let descriptor = merge_marks(mark1, mark2);
            union(p1, p2, Descriptor::Unbound(descriptor));
            Ok(())
        }
    }
}

fn merge_marks(mark1: FlexMark, mark2: FlexMark) -> FlexMark {
    match (mark1, mark2) {
        (FlexMark::Named(name), _) => FlexMark::Named(name),
        (_, FlexMark::Named(name)) => FlexMark::Named(name),
        (FlexMark::Fresh(id), FlexMark::Fresh(_)) => FlexMark::Fresh(id),
    }
}

fn bind_variable(span: &Span, point: &Point<Descriptor>, ty: &Type) -> Result<(), TypeError> {
    let ty = canonical_type(ty);
    if occurs(point, &ty) {
        return Err(TypeError::OccursCheck { span: *span, ty });
    }
    set(point, Descriptor::Bound(Box::new(ty)));
    Ok(())
}

fn occurs(point: &Point<Descriptor>, ty: &Type) -> bool {
    match canonical_type(ty) {
        Type::Var(p) => &p == point,
        Type::Prim(_) => false,
        Type::Fun(args, res) => {
            for arg in args {
                if occurs(point, &arg) {
                    return true;
                }
            }
            occurs(point, &res)
        }
        Type::Array(elem) => occurs(point, &elem),
        Type::Record(row_point) => occurs_in_row(point, &row_point),
        Type::DiscriminatedUnion(map) => {
            for (_, rp) in map {
                if occurs_in_row(point, &rp) {
                    return true;
                }
            }
            false
        }
    }
}

fn occurs_in_row(point: &Point<Descriptor>, row_point: &Point<RowDescriptor>) -> bool {
    let desc = get_row(row_point);
    match desc {
        RowDescriptor::RowFlex(_) => false,
        RowDescriptor::RowExtend(fields, tail) => {
            for (_, ty) in fields {
                if occurs(point, &ty) {
                    return true;
                }
            }
            occurs_in_row(point, &tail)
        }
    }
}

fn gather_fields(
    mut fields: BTreeMap<Name, Type>,
    row_point: &Point<RowDescriptor>,
) -> (BTreeMap<Name, Type>, Point<RowDescriptor>) {
    let mut current = row_point.clone();
    loop {
        let desc = get_row(&current);
        match desc {
            RowDescriptor::RowExtend(sub_fields, sub_ext) => {
                // Union fields, with existing fields taking precedence
                for (name, ty) in sub_fields {
                    fields.entry(name).or_insert(ty);
                }
                current = sub_ext;
            }
            RowDescriptor::RowFlex(_) => {
                break;
            }
        }
    }
    (fields, current)
}

fn unify_rows(
    ctx: &mut InferContext,
    span: &Span,
    r1: &Point<RowDescriptor>,
    r2: &Point<RowDescriptor>,
) -> Result<(), TypeError> {
    if r1 == r2 {
        return Ok(());
    }

    let desc1 = get_row(r1);
    let desc2 = get_row(r2);

    match (desc1, desc2) {
        (RowDescriptor::RowFlex(mark1), RowDescriptor::RowFlex(mark2)) => {
            let mark = merge_marks(mark1, mark2);
            union(r1, r2, RowDescriptor::RowFlex(mark));
            Ok(())
        }
        (RowDescriptor::RowFlex(_), desc) | (desc, RowDescriptor::RowFlex(_)) => {
            if occurs_row_check(r1, &desc) || occurs_row_check(r2, &desc) {
                return Err(TypeError::RowMismatch {
                    span: *span,
                    message: "occurs check failed".to_string(),
                });
            }
            union(r1, r2, desc);
            Ok(())
        }
        (RowDescriptor::RowExtend(_, _), RowDescriptor::RowExtend(_, _)) => {
            // Gather all fields recursively
            let structure1 = gather_fields(BTreeMap::new(), r1);
            let structure2 = gather_fields(BTreeMap::new(), r2);
            unify_record_structure(ctx, span, structure1, structure2)
        }
    }
}

fn unify_record_structure(
    ctx: &mut InferContext,
    span: &Span,
    (fields1, ext1): (BTreeMap<Name, Type>, Point<RowDescriptor>),
    (fields2, ext2): (BTreeMap<Name, Type>, Point<RowDescriptor>),
) -> Result<(), TypeError> {
    // Find shared and unique fields
    let mut unique_fields1 = BTreeMap::new();
    let mut unique_fields2 = fields2.clone();

    for (name, ty1) in fields1 {
        if let Some(ty2) = unique_fields2.remove(&name) {
            unify(ctx, span, &ty1, &ty2)?;
        } else {
            unique_fields1.insert(name, ty1);
        }
    }

    // Unify based on which sets are empty
    if unique_fields1.is_empty() {
        if unique_fields2.is_empty() {
            unify_rows(ctx, span, &ext1, &ext2)?;
        } else {
            let sub_record = ctx.fresh_row_extend(unique_fields2, ext2);
            unify_rows(ctx, span, &ext1, &sub_record)?;
        }
    } else if unique_fields2.is_empty() {
        let sub_record = ctx.fresh_row_extend(unique_fields1, ext1);
        unify_rows(ctx, span, &sub_record, &ext2)?;
    } else {
        let ext = ctx.fresh_row_point();
        let sub1 = ctx.fresh_row_extend(unique_fields1, ext.clone());
        let sub2 = ctx.fresh_row_extend(unique_fields2, ext.clone());

        unify_rows(ctx, span, &ext1, &sub2)?;
        unify_rows(ctx, span, &sub1, &ext2)?;
    };

    Ok(())
}

fn occurs_row_check(row_point: &Point<RowDescriptor>, desc: &RowDescriptor) -> bool {
    match desc {
        RowDescriptor::RowFlex(_) => false,
        RowDescriptor::RowExtend(fields, tail) => {
            // Check if row_point occurs in any field types
            for ty in fields.values() {
                if occurs_in_row_type(row_point, ty) {
                    return true;
                }
            }
            // Recursively check in the tail
            if tail == row_point {
                return true;
            }
            let tail_desc = get_row(tail);
            occurs_row_check(row_point, &tail_desc)
        }
    }
}

fn occurs_in_row_type(row_point: &Point<RowDescriptor>, ty: &Type) -> bool {
    match ty {
        Type::Var(_) => false,
        Type::Prim(_) => false,
        Type::Fun(args, res) => {
            for arg in args {
                if occurs_in_row_type(row_point, arg) {
                    return true;
                }
            }
            occurs_in_row_type(row_point, res)
        }
        Type::Array(elem) => occurs_in_row_type(row_point, elem),
        Type::Record(rp) => {
            if rp == row_point {
                return true;
            }
            let desc = get_row(rp);
            occurs_row_check(row_point, &desc)
        }
        Type::DiscriminatedUnion(map) => {
            for rp in map.values() {
                if rp == row_point {
                    return true;
                }
                let desc = get_row(rp);
                if occurs_row_check(row_point, &desc) {
                    return true;
                }
            }
            false
        }
    }
}
