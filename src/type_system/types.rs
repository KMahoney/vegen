use std::collections::BTreeMap;

use crate::{ast::Span, type_system::uf::Point};

pub type Name = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Prim(String),
    Fun(Vec<Type>, Box<Type>),
    Array(Box<Type>),
    Var(Point<Descriptor>),
    Record(Point<RowDescriptor>),
    DiscriminatedUnion(BTreeMap<String, Point<RowDescriptor>>),
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Prim(name) => write!(f, "{}", name),
            Type::Fun(args, ret) => {
                let arg_strings: Vec<String> = args.iter().map(|arg| format!("{}", arg)).collect();
                write!(f, "({}) -> {}", arg_strings.join(", "), ret)
            }
            Type::Array(elem) => write!(f, "Array<{}>", elem),
            Type::Var(point) => write!(f, "{}", point),
            Type::Record(point) => write!(f, "{{{}}}", point),
            Type::DiscriminatedUnion(map) => {
                // Render as { type: "a", ...rest } | { type: "b", ...rest } | ...
                let mut arms: Vec<String> = Vec::new();
                for (k, rp) in map {
                    arms.push(format!("{{ type: \"{}\", ...{} }}", k, rp));
                }
                write!(f, "{}", arms.join(" | "))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Descriptor {
    Unbound(FlexMark),
    Bound(Box<Type>),
}

impl std::fmt::Display for Descriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Descriptor::Unbound(mark) => write!(f, "{}", mark),
            Descriptor::Bound(ty) => write!(f, "{}", ty),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowDescriptor {
    RowExtend(BTreeMap<Name, Type>, Point<RowDescriptor>),
    RowFlex(FlexMark),
}

impl std::fmt::Display for RowDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RowDescriptor::RowExtend(fields, rest) => {
                let mut field_strings: Vec<String> = fields
                    .iter()
                    .map(|(name, ty)| format!("{}: {}", name, ty))
                    .collect();
                field_strings.push(format!("...{}", rest));
                write!(f, "{}", field_strings.join(", "))
            }
            RowDescriptor::RowFlex(mark) => write!(f, "R{}", mark),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlexMark {
    Fresh(usize),
    Named(Name),
}

impl std::fmt::Display for FlexMark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlexMark::Fresh(id) => write!(f, "'{}", id),
            FlexMark::Named(name) => write!(f, "'{}", name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint {
    Equal(Span, Type, Type),
}

impl std::fmt::Display for Constraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Constraint::Equal(_, t1, t2) => write!(f, "{} == {}", t1, t2),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected {
    _NoExpect,
    Expect(Type),
}
