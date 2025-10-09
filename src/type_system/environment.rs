use std::collections::HashMap;

use crate::builtins::{BuiltinType, BUILTINS};
use crate::type_system::types::{Descriptor, FlexMark, Name, RowDescriptor};
use crate::type_system::uf::{fresh, Point};
use crate::type_system::Type;

#[derive(Debug, Default)]
pub struct InferContext {
    next_id: usize,
    next_row_id: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Env {
    scopes: Vec<HashMap<Name, Type>>,
    globals: HashMap<Name, Type>,
}

impl Env {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_scope(&mut self, scope: HashMap<Name, Type>) {
        self.scopes.push(scope);
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn get(&mut self, ctx: &mut InferContext, name: &Name) -> Type {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return ty.clone();
            }
        }
        self.get_global(name)
            .cloned()
            .or_else(|| self.get_builtin(ctx, name).map(Type::Var))
            .unwrap_or_else(|| {
                let point = ctx.fresh_named(name);
                let ty = Type::Var(point.clone());
                self.globals.insert(name.clone(), ty.clone());
                ty
            })
    }

    fn get_global(&self, name: &Name) -> Option<&Type> {
        self.globals.get(name)
    }

    fn get_builtin(&self, ctx: &mut InferContext, name: &Name) -> Option<Point<Descriptor>> {
        BUILTINS.get(name).map(|(args, ret)| {
            let mut vars = HashMap::new();

            let args = args
                .iter()
                .map(|arg| instantiate(arg, &mut vars, ctx))
                .collect();

            let ret = instantiate(ret, &mut vars, ctx);

            ctx.fresh_descriptor(Descriptor::Bound(Box::new(Type::Fun(args, Box::new(ret)))))
        })
    }
}

fn instantiate(
    builtin_type: &BuiltinType,
    vars: &mut HashMap<u32, Point<Descriptor>>,
    ctx: &mut InferContext,
) -> Type {
    match builtin_type {
        BuiltinType::Prim(name) => Type::Prim(name.clone()),
        BuiltinType::Var(id) => {
            if let Some(point) = vars.get(id) {
                Type::Var(point.clone())
            } else {
                let point = ctx.fresh_point();
                vars.insert(*id, point.clone());
                Type::Var(point)
            }
        }
    }
}

impl<'a> IntoIterator for &'a Env {
    type Item = (&'a Name, &'a Type);
    type IntoIter = std::collections::hash_map::Iter<'a, Name, Type>;

    fn into_iter(self) -> Self::IntoIter {
        self.globals.iter()
    }
}

impl InferContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fresh_point(&mut self) -> Point<Descriptor> {
        let id = self.allocate_id();
        let mark = FlexMark::Fresh(id);
        fresh(id, Descriptor::Unbound(mark))
    }

    pub fn fresh_descriptor(&mut self, desc: Descriptor) -> Point<Descriptor> {
        let id = self.allocate_id();
        fresh(id, desc)
    }

    pub fn fresh_named(&mut self, name: &Name) -> Point<Descriptor> {
        let id = self.allocate_id();
        fresh(id, Descriptor::Unbound(FlexMark::Named(name.clone())))
    }

    pub fn fresh_row_point(&mut self) -> Point<RowDescriptor> {
        let id = self.allocate_row_id();
        let mark = FlexMark::Fresh(id);
        fresh(id, RowDescriptor::RowFlex(mark))
    }

    pub fn fresh_row_extend(
        &mut self,
        fields: std::collections::BTreeMap<Name, crate::type_system::types::Type>,
        ext: Point<RowDescriptor>,
    ) -> Point<RowDescriptor> {
        let id = self.allocate_row_id();
        fresh(id, RowDescriptor::RowExtend(fields, ext))
    }

    fn allocate_id(&mut self) -> usize {
        let current = self.next_id;
        self.next_id += 1;
        current
    }

    fn allocate_row_id(&mut self) -> usize {
        let current = self.next_row_id;
        self.next_row_id += 1;
        current
    }
}
