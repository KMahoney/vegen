use crate::ast::{AttrValue, Node, Span, SpannedAttribute};
use crate::ast_query::{
    collect_attr_dependencies, expect_element, find_binding_attr, find_literal_attr,
    find_unique_child_by_name, has_bindings, infer_attr_type, split_data_attribute,
    validate_all_children_are_elements, validate_child_element_names, validate_single_child,
};
use crate::emit::emit_views;
use crate::error::Error;
use crate::expr::{expr_dependencies, Expr, StringTemplateSegment};
use crate::ir::{
    CompileContext, CompiledView, ForLoopInfo, IfInfo, JsExpr, JsUpdater, SwitchInfo, UpdateKind,
    ViewDefinition,
};
use crate::ts_type::{env_to_ts_type, TsType};
use crate::type_system::environment::{Env, InferContext, TypeMap};
use crate::type_system::infer::infer;
use crate::type_system::solver::solve;
use crate::type_system::types::{Constraint, Expected};
use crate::type_system::Type;
use chumsky::span::SimpleSpan;
use itertools::Itertools;
use std::collections::{BTreeMap, HashMap, HashSet};

struct TypeEnv {
    env: Env,
    infer_ctx: InferContext,
    constraints: Vec<Constraint>,
    views: HashMap<String, TypeMap>,
}

impl TypeEnv {
    fn new() -> Self {
        Self {
            env: Env::new(),
            infer_ctx: InferContext::new(),
            constraints: Vec::new(),
            views: HashMap::new(),
        }
    }

    fn infer(&mut self, expr: &Expr, expected: Expected) {
        infer(
            &mut self.infer_ctx,
            &mut self.env,
            &mut self.constraints,
            expr,
            expected,
        );
    }

    fn solve_view(&mut self, view_name: String) -> Result<TsType, Error> {
        solve(&mut self.infer_ctx, &self.constraints).map_err(|e| e.to_error())?;
        let ts_type = env_to_ts_type(&self.env);
        self.views.insert(view_name, self.env.globals().clone());
        self.env = Env::new();
        self.constraints = Vec::new();
        Ok(ts_type)
    }
}

#[derive(Debug)]
struct ViewInfo {
    name: String,
    node: Node,
    span: Span,
}

pub fn compile(nodes: &[Node]) -> Result<String, Error> {
    let mut env = TypeEnv::new();
    let mut views_info = Vec::new();
    let mut component_deps = HashMap::new();

    for node in nodes {
        let (attrs, children, span) = expect_element(node, "view")?;
        let (view_name, name_span) = find_literal_attr(attrs, "name", span)?;

        if !view_name.chars().next().unwrap_or(' ').is_uppercase() {
            return Err(Error {
                message: "View names must start with a capital letter".to_string(),
                main_span: name_span,
                labels: vec![(name_span, "View name should start with capital".to_string())],
            });
        }

        validate_single_child(span, children)?;

        // Find component references in this view
        let mut component_refs = HashSet::new();
        find_component_refs(&children[0], &mut component_refs);

        views_info.push(ViewInfo {
            name: view_name.clone(),
            node: children[0].clone(),
            span: *span,
        });

        component_deps.insert(view_name, component_refs);
    }

    let defined_views: HashSet<String> = component_deps.keys().cloned().collect();
    let component_deps: HashMap<String, HashSet<String>> = component_deps
        .into_iter()
        .map(|(view, deps)| {
            (
                view,
                deps.into_iter()
                    .filter(|d| defined_views.contains(d))
                    .collect(),
            )
        })
        .collect();

    let compilation_order = topological_sort(&component_deps)?;
    let mut compiled_views = Vec::new();

    for view_name in compilation_order {
        let view_info = views_info
            .iter()
            .find(|v| v.name == view_name)
            .expect("internal error: view not found");

        let mut context = CompileContext::new();
        let root = compile_view(&view_info.node, &mut context, &mut env, view_info.span)?;
        let ts_type = env.solve_view(view_name.clone())?;

        compiled_views.push(ViewDefinition {
            view_name: view_name.clone(),
            root,
            context,
            ts_type,
        });
    }

    Ok(emit_views(&compiled_views))
}

fn find_component_refs(node: &Node, refs: &mut HashSet<String>) {
    match node {
        Node::ComponentCall { name, children, .. } => {
            refs.insert(name.clone());
            // Recursively check children
            for child in children {
                find_component_refs(child, refs);
            }
        }
        Node::Element { children, .. } => {
            for child in children {
                find_component_refs(child, refs);
            }
        }
        Node::Text { .. } | Node::Expr { .. } => {}
    }
}

fn topological_sort(deps: &HashMap<String, HashSet<String>>) -> Result<Vec<String>, Error> {
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();

    fn visit(
        node: &str,
        deps: &HashMap<String, HashSet<String>>,
        order: &mut Vec<String>,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
    ) -> Result<(), Error> {
        if visited.contains(node) {
            return Ok(());
        }
        if visiting.contains(node) {
            return Err(Error {
                message: format!("Circular component dependency involving '{}'", node),
                // FIXME
                main_span: SimpleSpan {
                    start: 0,
                    end: 1,
                    context: 0,
                },
                labels: vec![],
            });
        }

        visiting.insert(node.to_string());

        if let Some(node_deps) = deps.get(node) {
            for dep in node_deps.iter().sorted() {
                visit(dep, deps, order, visited, visiting)?;
            }
        }

        visiting.remove(node);
        visited.insert(node.to_string());
        order.push(node.to_string());

        Ok(())
    }

    for node in deps.keys().sorted() {
        visit(node, deps, &mut order, &mut visited, &mut visiting)?;
    }

    Ok(order)
}

fn compile_view(
    node: &Node,
    context: &mut CompileContext,
    env: &mut TypeEnv,
    view_span: Span,
) -> Result<JsExpr, Error> {
    let expr = compile_node(node, context, env)?;

    if matches!(expr, JsExpr::LoopElements(_)) {
        return Err(Error {
            message: "<for> elements cannot be root elements; wrap them in a container."
                .to_string(),
            main_span: view_span,
            labels: vec![
                (view_span, "View".to_string()),
                (*node.span(), "element".to_string()),
            ],
        });
    }

    Ok(expr)
}

fn compile_node(
    node: &Node,
    context: &mut CompileContext,
    env: &mut TypeEnv,
) -> Result<JsExpr, Error> {
    match node {
        Node::Element {
            name,
            attrs,
            children,
            span,
            ..
        } => {
            if name == "for" {
                compile_for_loop(attrs, children, span, context, env)
            } else if name == "if" {
                compile_if(attrs, children, span, context, env)
            } else if name == "switch" {
                compile_switch(attrs, children, span, context, env)
            } else if name == "mount" {
                compile_mount(attrs, span, context, env)
            } else {
                compile_element(name, attrs, children, context, env)
            }
        }
        Node::ComponentCall {
            name,
            attrs,
            children,
            span,
            ..
        } => compile_component_call(name, attrs, children, span, context, env),
        Node::Text { content, .. } => Ok(JsExpr::Text(content.clone())),
        Node::Expr(expr) => {
            let binding_expr = JsExpr::Expr(expr.clone());
            let node_idx = context.constructors.len();
            context.constructors.push(binding_expr.clone());

            context.updaters.push(JsUpdater {
                dependencies: expr_dependencies(expr).into_iter().collect(),
                kind: UpdateKind::Text {
                    node_idx,
                    value: AttrValue::Expr(expr.clone()),
                },
            });

            env.infer(expr, Expected::Expect(Type::Prim("string".to_string())));

            Ok(JsExpr::Ref(node_idx))
        }
    }
}

fn compile_element(
    name: &str,
    attrs: &[SpannedAttribute],
    children: &[Node],
    context: &mut CompileContext,
    env: &mut TypeEnv,
) -> Result<JsExpr, Error> {
    let mut props: Vec<(String, AttrValue)> = Vec::new();
    let mut dataset: Vec<(String, AttrValue)> = Vec::new();
    let mut prop_updaters: Vec<JsUpdater> = Vec::new();
    let mut child_exprs = Vec::new();
    for child in children {
        let expr = compile_node(child, context, env)?;
        child_exprs.push(expr);
    }
    let node_idx = context.constructors.len();
    for attr in attrs {
        let k = &attr.name;
        let v = &attr.value;

        // Map attribute name to DOM property name where appropriate (e.g., class -> className)
        let dom_prop_name = if k == "class" {
            "className".to_string()
        } else {
            k.clone()
        };

        // Determine if this is a data attribute and get the appropriate key
        let attr_key = if let Some(dataset_key) = split_data_attribute(k) {
            dataset.push((dataset_key.clone(), v.clone()));
            dataset_key
        } else {
            props.push((dom_prop_name.clone(), v.clone()));
            k.clone()
        };

        let is_data_attr = split_data_attribute(k).is_some();

        // Infer types for bindings
        match v {
            AttrValue::Template(segments) => {
                for seg in segments {
                    if let StringTemplateSegment::Interpolation(expr) = seg {
                        env.infer(expr, Expected::Expect(Type::Prim("string".to_string())));
                    }
                }
            }
            AttrValue::Expr(expr) => {
                let ty = if is_data_attr {
                    "string".to_string()
                } else {
                    infer_attr_type(k, name)
                };
                env.infer(expr, Expected::Expect(Type::Prim(ty)));
            }
        }

        // Create updater if attribute has dynamic content
        if has_bindings(v) {
            let deps = collect_attr_dependencies(v);
            if is_data_attr {
                prop_updaters.push(JsUpdater {
                    dependencies: deps,
                    kind: UpdateKind::Dataset {
                        node_idx,
                        key: attr_key,
                        value: v.clone(),
                    },
                });
            } else {
                prop_updaters.push(JsUpdater {
                    dependencies: deps,
                    kind: UpdateKind::Prop {
                        node_idx,
                        // Use the DOM property name so updates write to e.g. node["className"]
                        prop: dom_prop_name.clone(),
                        value: v.clone(),
                    },
                });
            }
        }
    }
    let element_expr = JsExpr::Element {
        tag: name.to_string(),
        props,
        dataset,
        children: child_exprs,
    };
    if !prop_updaters.is_empty() {
        context.constructors.push(element_expr);
        context.updaters.extend(prop_updaters);
        Ok(JsExpr::Ref(node_idx))
    } else {
        // We don't need to reference this node in an updater, so just inline the element expression
        Ok(element_expr)
    }
}

fn compile_for_loop(
    attrs: &[SpannedAttribute],
    children: &[Node],
    span: &Span,
    context: &mut CompileContext,
    env: &mut TypeEnv,
) -> Result<JsExpr, Error> {
    let seq = find_binding_attr(attrs, "seq", span)?;
    let (var, _) = find_literal_attr(attrs, "as", span)?;
    validate_single_child(span, children)?;

    let mut sub_context = CompileContext::new();
    let array_type = env.infer_ctx.fresh_point();
    let mut scope = HashMap::new();
    scope.insert(var.clone(), Type::Var(array_type.clone()));
    env.env.push_scope(scope);
    let child_root = compile_view(&children[0], &mut sub_context, env, *span)?;
    env.env.pop_scope();
    env.infer(
        &seq,
        Expected::Expect(Type::Array(Box::new(Type::Var(array_type)))),
    );
    let child_view_idx = context.child_views.len();
    context.child_views.push(CompiledView {
        root: child_root,
        context: sub_context,
    });

    // Track for loop information with outer scope dependencies
    context.for_loops.push(ForLoopInfo {
        child_view_idx,
        sequence_expr: seq.clone(),
        var_name: var.clone(),
    });

    // Return spread of loop elements
    Ok(JsExpr::LoopElements(context.for_loops.len() - 1))
}

fn compile_if(
    attrs: &[SpannedAttribute],
    children: &[Node],
    span: &Span,
    context: &mut CompileContext,
    env: &mut TypeEnv,
) -> Result<JsExpr, Error> {
    let condition = find_binding_attr(attrs, "condition", span)?;

    // Validate children are all elements with expected names
    validate_all_children_are_elements(span, children)?;
    validate_child_element_names(span, children, &["then", "else"])?;

    // Find unique <then> and <else> children
    let then_child = find_unique_child_by_name(children, "then", span)?;
    let else_child = find_unique_child_by_name(children, "else", span)?;

    // Validate that at least one of then or else is present
    if then_child.is_none() && else_child.is_none() {
        return Err(Error {
            message: "Missing <then> and <else> blocks in <if>; at least one must be present."
                .to_string(),
            main_span: *span,
            labels: vec![(*span, "Missing <then> or <else> block".to_string())],
        });
    }

    let mut then_view_idx: Option<usize> = None;
    let mut else_view_idx: Option<usize> = None;

    // Compile then branch if present
    if let Some(then_node) = then_child {
        let (_, children, _) = expect_element(then_node, "then")?;
        validate_single_child(then_node.span(), children)?;
        let mut then_context = CompileContext::new();
        let then_root = compile_view(&children[0], &mut then_context, env, *span)?;
        then_view_idx = Some(context.child_views.len());
        context.child_views.push(CompiledView {
            root: then_root,
            context: then_context,
        });
    }

    // Compile else branch if present
    if let Some(else_node) = else_child {
        let (_, children, _) = expect_element(else_node, "else")?;
        validate_single_child(else_node.span(), children)?;
        let mut else_context = CompileContext::new();
        let else_root = compile_view(&children[0], &mut else_context, env, *span)?;
        else_view_idx = Some(context.child_views.len());
        context.child_views.push(CompiledView {
            root: else_root,
            context: else_context,
        });
    }

    env.infer(
        &condition,
        Expected::Expect(Type::Prim("boolean".to_string())),
    );

    // Track if information
    context.ifs.push(IfInfo {
        then_view_idx,
        else_view_idx,
        condition_expr: condition.clone(),
    });

    // Return conditional element
    Ok(JsExpr::ConditionalElement(context.ifs.len() - 1))
}

fn compile_switch(
    attrs: &[SpannedAttribute],
    children: &[Node],
    span: &Span,
    context: &mut CompileContext,
    env: &mut TypeEnv,
) -> Result<JsExpr, Error> {
    // Validate 'on' binding
    let on_binding = find_binding_attr(attrs, "on", span)?;

    // Validate children are all <case> elements
    validate_all_children_are_elements(span, children)?;
    validate_child_element_names(span, children, &["case"])?;

    if children.is_empty() {
        return Err(Error {
            message: "Missing <case> blocks in <switch>; at least one must be present.".to_string(),
            main_span: *span,
            labels: vec![(*span, "No <case> blocks".to_string())],
        });
    }

    // Track case view indices and names
    let mut case_view_idxs: Vec<usize> = Vec::new();
    let mut case_names: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Map of case name -> tail row point for union construction
    let mut union_map: BTreeMap<
        String,
        crate::type_system::uf::Point<crate::type_system::types::RowDescriptor>,
    > = BTreeMap::new();

    for case_node in children {
        let (case_attrs, case_children, case_span) = expect_element(case_node, "case")?;
        // Each case must have exactly one child
        validate_single_child(case_span, case_children)?;
        // Each case must have a literal name
        let (name, _) = find_literal_attr(case_attrs, "name", case_span)?;
        if !seen.insert(name.clone()) {
            return Err(Error {
                message: format!("Duplicate case name '{}' in <switch>", name),
                main_span: *span,
                labels: vec![(*case_span, "Duplicate case".to_string())],
            });
        }

        // Bind alias with narrowed record type { type: "name", ...RowVar }
        let tail = env.infer_ctx.fresh_row_point();
        let mut fields = BTreeMap::new();
        fields.insert("type".to_string(), Type::Prim(format!("\"{}\"", name)));
        let row = env.infer_ctx.fresh_row_extend(fields, tail.clone());
        let alias_ty = Type::Record(row);

        // Push scope with alias name bound to alias_ty
        let point = env.infer_ctx.fresh_point();
        let mut scope = HashMap::new();
        scope.insert(name.clone(), Type::Var(point.clone()));
        env.env.push_scope(scope);
        env.constraints
            .push(Constraint::Equal(*case_span, Type::Var(point), alias_ty));

        // Compile case body as a child view
        let mut sub_context = CompileContext::new();
        let child_root = compile_view(&case_children[0], &mut sub_context, env, *case_span)?;
        env.env.pop_scope();

        let child_view_idx = context.child_views.len();
        context.child_views.push(CompiledView {
            root: child_root,
            context: sub_context,
        });

        case_view_idxs.push(child_view_idx);
        case_names.push(name.clone());
        union_map.insert(name, tail);
    }

    // Unify the 'on' expression with a discriminated union of the collected cases
    env.infer(
        &on_binding,
        Expected::Expect(Type::DiscriminatedUnion(union_map)),
    );

    // Track switch info in context
    context.switches.push(SwitchInfo {
        case_view_idxs,
        case_names,
        on_expr: on_binding.clone(),
    });

    Ok(JsExpr::SwitchElement(context.switches.len() - 1))
}

fn compile_mount(
    attrs: &[SpannedAttribute],
    span: &Span,
    context: &mut CompileContext,
    env: &mut TypeEnv,
) -> Result<JsExpr, Error> {
    let use_binding = find_binding_attr(attrs, "use", span)?;

    env.infer(
        &use_binding,
        Expected::Expect(Type::Prim("() => Element".to_string())),
    );

    // Collect mount binding and dependencies
    let mount_idx = context.mounts.len();
    let dependencies = expr_dependencies(&use_binding).into_iter().collect();
    context.mounts.push(crate::ir::MountInfo {
        use_expr: use_binding.clone(),
        dependencies,
    });

    Ok(JsExpr::Mount(mount_idx))
}

fn compile_component_call(
    name: &str,
    attrs: &[SpannedAttribute],
    _children: &[Node],
    span: &Span,
    context: &mut CompileContext,
    env: &mut TypeEnv,
) -> Result<JsExpr, Error> {
    // Check if component exists in stored views
    let view_attrs = env.views.get(name).cloned().ok_or_else(|| Error {
        message: format!("Component '{}' not found", name),
        main_span: *span,
        labels: vec![(*span, format!("Component '{}' is used here", name))],
    })?;

    // Build map of provided attributes
    let provided_attrs: std::collections::HashMap<_, _> = attrs
        .iter()
        .map(|attr| {
            let attr_expr = match &attr.value {
                crate::ast::AttrValue::Template(segments) => {
                    crate::expr::Expr::StringTemplate(segments.clone(), *span)
                }
                crate::ast::AttrValue::Expr(expr) => expr.clone(),
            };
            (attr.name.clone(), attr_expr)
        })
        .collect();

    let required_keys: HashSet<String> = view_attrs.keys().cloned().collect();
    let provided_keys: HashSet<String> = provided_attrs.keys().cloned().collect();

    // Check for missing attributes
    let missing_attrs: Vec<_> = required_keys.difference(&provided_keys).cloned().collect();

    if !missing_attrs.is_empty() {
        return Err(Error {
            message: format!(
                "Component '{}' is missing required attributes: {}",
                name,
                missing_attrs.join(", ")
            ),
            main_span: *span,
            labels: vec![],
        });
    }

    // Check for extra attributes
    let extra_attrs: Vec<_> = provided_keys.difference(&required_keys).cloned().collect();

    if !extra_attrs.is_empty() {
        return Err(Error {
            message: format!(
                "Component '{}' has unexpected attributes: {}",
                name,
                extra_attrs.join(", ")
            ),
            main_span: *span,
            labels: vec![],
        });
    }

    // Type check each attribute
    let instantiated_view_attrs = env.infer_ctx.instantiate_attrs(&view_attrs);
    for (attr_name, attr_expr) in &provided_attrs {
        let ty = instantiated_view_attrs.get(attr_name).unwrap();
        env.infer(attr_expr, Expected::Expect(ty.clone()));
    }

    // Create component call info
    let component_idx = context.component_calls.len();
    context.component_calls.push(crate::ir::ComponentCallInfo {
        target_view_name: name.to_string(),
        input_attrs: provided_attrs,
    });

    Ok(JsExpr::ComponentCall(component_idx))
}
