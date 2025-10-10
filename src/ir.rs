use crate::{
    ast::{AttrValue, SpannedBinding},
    expr::Expr,
    ts_type::TsType,
};

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ViewDefinition {
    pub view_name: String,
    pub context: CompileContext,
    pub ts_type: TsType,
    pub root: JsExpr,
}

#[derive(Debug, Clone)]
pub struct CompileContext {
    pub constructors: Vec<JsExpr>,
    pub updaters: Vec<JsUpdater>,
    pub child_views: Vec<CompiledView>,
    pub for_loops: Vec<ForLoopInfo>,
    pub ifs: Vec<IfInfo>,
    pub switches: Vec<SwitchInfo>,
    pub use_views: Vec<UseViewInfo>,
    pub component_calls: Vec<ComponentCallInfo>,
    pub mounts: Vec<Expr>,
}

impl CompileContext {
    pub fn new() -> Self {
        Self {
            constructors: Vec::new(),
            updaters: Vec::new(),
            child_views: Vec::new(),
            for_loops: Vec::new(),
            ifs: Vec::new(),
            switches: Vec::new(),
            use_views: Vec::new(),
            component_calls: Vec::new(),
            mounts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompiledView {
    pub root: JsExpr,
    pub context: CompileContext,
}

#[derive(Debug, Clone)]
pub enum JsExpr {
    Element {
        tag: String,
        props: Vec<(String, AttrValue)>,
        dataset: Vec<(String, AttrValue)>,
        children: Vec<JsExpr>,
    },
    Text(String),
    Binding(SpannedBinding),
    Ref(usize),
    LoopElements(usize),
    ConditionalElement(usize),
    SwitchElement(usize),
    Mount(usize),
    UseView(usize),
    ComponentCall(usize),
}

#[derive(Debug, Clone)]
pub enum UpdateKind {
    Text {
        node_idx: usize,
        binding: AttrValue,
    },
    Prop {
        node_idx: usize,
        prop: String,
        binding: AttrValue,
    },
    Dataset {
        node_idx: usize,
        key: String,
        binding: AttrValue,
    },
}

#[derive(Debug, Clone)]
pub struct JsUpdater {
    pub dependencies: Vec<String>,
    pub kind: UpdateKind,
}

#[derive(Debug, Clone)]
pub struct ForLoopInfo {
    pub child_view_idx: usize,
    pub sequence_expr: Expr,
    pub var_name: String,
}

#[derive(Debug, Clone)]
pub struct IfInfo {
    pub then_view_idx: Option<usize>,
    pub else_view_idx: Option<usize>,
    pub condition_expr: Expr,
}

#[derive(Debug, Clone)]
pub struct SwitchInfo {
    pub case_view_idxs: Vec<usize>,
    pub case_names: Vec<String>,
    pub on_expr: Expr,
}

#[derive(Debug, Clone)]
pub struct UseViewInfo {
    pub target_view_name: String,
    pub input_expr: Expr,
}

#[derive(Debug, Clone)]
pub struct ComponentCallInfo {
    pub target_view_name: String,
    pub input_attrs: HashMap<String, Expr>,
}
