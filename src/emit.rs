use crate::ast::AttrValue;
use crate::builtins::BUILTINS;
use crate::expr::{self, StringTemplateSegment};
use crate::ir::{CompiledView, JsExpr, JsUpdater, UpdateKind, ViewDefinition};
use crate::ts_util::render_key;
use itertools::Itertools;
use std::collections::BTreeMap;

pub fn render(view: &CompiledView, indent: &str) -> String {
    let CompiledView {
        root,
        context: view,
    } = view;
    fn serialize_js_expr(expr: &JsExpr) -> String {
        match expr {
            JsExpr::Element {
                tag,
                props,
                dataset,
                children,
            } => {
                let props_str = if props.is_empty() {
                    "{}".to_string()
                } else {
                    let pairs = props
                        .iter()
                        .sorted_by(|(a, _), (b, _)| a.cmp(b))
                        .map(|(k, v)| format!("{}: {}", render_key(k), render_attr_value(v)))
                        .join(", ");
                    format!("{{{}}}", pairs)
                };
                let children_str = children.iter().map(serialize_js_expr).join(", ");

                if dataset.is_empty() {
                    format!("h(\"{}\", {}, [{}])", tag, props_str, children_str)
                } else {
                    let dataset_str = {
                        let pairs = dataset
                            .iter()
                            .sorted_by(|(a, _), (b, _)| a.cmp(b))
                            .map(|(k, v)| format!("{}: {}", render_key(k), render_attr_value(v)))
                            .join(", ");
                        format!("{{{}}}", pairs)
                    };
                    format!(
                        "h(\"{}\", {}, [{}], {})",
                        tag, props_str, children_str, dataset_str
                    )
                }
            }
            JsExpr::Text(text) => format!("t({:?})", text),
            JsExpr::Expr(expr) => {
                format!("t({})", render_attr_value(&AttrValue::Expr(expr.clone())))
            }
            JsExpr::Ref(idx) => format!("node{}", idx),
            JsExpr::LoopElements(idx) => format!("...loopElements{}", idx),
            JsExpr::ConditionalElement(idx) => format!("conditionalElement{}", idx),
            JsExpr::SwitchElement(idx) => format!("switchElement{}", idx),
            JsExpr::Use(idx) => format!("useViewState{}.root", idx),
            JsExpr::ComponentCall(idx) => format!("componentState{}.root", idx),
        }
    }

    fn serialize_update(kind: &UpdateKind) -> String {
        match kind {
            UpdateKind::Text { node_idx, value } => {
                format!(
                    "node{}.textContent = {}",
                    node_idx,
                    render_attr_value(value)
                )
            }
            UpdateKind::Prop {
                node_idx,
                prop,
                value,
            } => {
                format!(
                    "node{}[\"{}\"] = {}",
                    node_idx,
                    prop,
                    render_attr_value(value)
                )
            }
            UpdateKind::Dataset {
                node_idx,
                key,
                value,
            } => {
                format!(
                    "node{}.dataset[\"{}\"] = {}",
                    node_idx,
                    key,
                    render_attr_value(value)
                )
            }
        }
    }

    // Helper to apply indentation to lines
    fn apply_indent(lines: &[String], base_indent: &str, extra_indent: &str) -> Vec<String> {
        lines
            .iter()
            .map(|line| format!("{}{}{}", base_indent, extra_indent, line))
            .collect()
    }

    // Build: create nodes from constructors, append root
    let mut build_lines = Vec::new();

    // Build child views
    for (i, v) in view.child_views.iter().enumerate() {
        let child_code = render(v, &format!("{}  ", indent));
        build_lines.push(format!(
            "const child{}: View<any> = (input) => {{\n{}\n{}  }};",
            i, child_code, indent
        ));
    }

    // Process for loops (populate loopElements)
    for (i, for_loop) in view.for_loops.iter().enumerate() {
        build_lines.push(format!(
            "const anchor{} = document.createComment(\"for-loop-{}\");",
            i, i
        ));
        build_lines.push(format!("const loopElements{} = [];", i));

        build_lines.push(format!(
            "let childState{}: any[] = [];",
            for_loop.child_view_idx
        ));
        build_lines.push(format!(
            "for (const item of {}) {{",
            render_expr(&for_loop.sequence_expr)
        ));

        let input_obj = format!("{{ ...input, {}: item }}", for_loop.var_name);

        build_lines.push(format!(
            "  const itemState = child{}({});",
            for_loop.child_view_idx, input_obj
        ));
        build_lines.push(format!("  loopElements{}.push(itemState.root);", i));
        build_lines.push(format!(
            "  childState{}.push(itemState);",
            for_loop.child_view_idx
        ));
        build_lines.push("}".to_string());
        build_lines.push(format!("loopElements{}.push(anchor{});", i, i));
    }

    // Process ifs (initialize current state and element)
    for (i, if_info) in view.ifs.iter().enumerate() {
        build_lines.push(format!("let currentState{}: ViewState<any>;", i));

        build_lines.push(format!("if ({}) {{", render_expr(&if_info.condition_expr)));
        if let Some(then_idx) = if_info.then_view_idx {
            build_lines.push(format!("  currentState{} = child{}(input);", i, then_idx));
        } else {
            build_lines.push(format!(
                "  currentState{} = {{ root: document.createComment(\"empty\"), update: (_: any) => {{}} }};",
                i
            ));
        }
        build_lines.push("} else {".to_string());
        if let Some(else_idx) = if_info.else_view_idx {
            build_lines.push(format!("  currentState{} = child{}(input);", i, else_idx));
        } else {
            build_lines.push(format!(
                "  currentState{} = {{ root: document.createComment(\"empty\"), update: (_: any) => {{}} }};",
                i
            ));
        }
        build_lines.push("}".to_string());
        build_lines.push(format!(
            "const conditionalElement{} = currentState{}.root;",
            i, i
        ));
    }

    // Process switches (initialize current state and element)
    for (i, switch_info) in view.switches.iter().enumerate() {
        build_lines.push(format!("let currentSwitchState{}: ViewState<any>;", i));
        build_lines.push(format!("const switchElement{} = (() => {{", i));
        build_lines.push(format!(
            "  const onValue = {}.type;",
            render_expr(&switch_info.on_expr)
        ));
        build_lines.push("  switch (onValue) {".to_string());
        for (j, case_name) in switch_info.case_names.iter().enumerate() {
            let case_idx = switch_info.case_view_idxs[j];
            build_lines.push(format!("    case \"{}\": {{", case_name));
            build_lines.push(format!(
                "      const caseInput = {{ ...input, {}: {} }};",
                case_name,
                render_expr(&switch_info.on_expr)
            ));
            build_lines.push(format!("      const st = child{}(caseInput);", case_idx));
            build_lines.push(format!("      currentSwitchState{} = st;", i));
            build_lines.push("      return st.root;".to_string());
            build_lines.push("    }".to_string());
        }
        build_lines.push("    default: {".to_string());
        build_lines.push(
            "      const st = { root: document.createComment(\"switch-empty\"), update: (_: any) => {} };"
                .to_string(),
        );
        build_lines.push(format!("      currentSwitchState{} = st;", i));
        build_lines.push("      return st.root;".to_string());
        build_lines.push("    }".to_string());
        build_lines.push("  }".to_string());
        build_lines.push("})();".to_string());
    }

    // Process use views
    for (i, use_info) in view.use_views.iter().enumerate() {
        build_lines.push(format!(
            "let useViewState{} = {}({});",
            i,
            render_expr(&use_info.view_expr),
            render_object(&use_info.input_attrs)
        ));
    }

    // Process component calls (instantiate component views)
    for (i, component_call) in view.component_calls.iter().enumerate() {
        build_lines.push(format!(
            "const componentState{} = {}({});",
            i,
            component_call.target_view_name,
            render_object(&component_call.input_attrs)
        ));
    }

    // Add remaining constructors
    for (i, expr) in view.constructors.iter().enumerate() {
        build_lines.push(format!("const node{} = {};", i, serialize_js_expr(expr)));
    }

    // Create root
    build_lines.push(format!("const root = {};", serialize_js_expr(root)));

    build_lines.push("let currentInput = input;".to_string());

    // Update: group updaters by dependencies
    let mut update_lines = Vec::new();
    let mut grouped: BTreeMap<Vec<String>, Vec<&JsUpdater>> = BTreeMap::new();
    for updater in &view.updaters {
        let mut deps = updater.dependencies.clone();
        deps.sort();
        grouped.entry(deps).or_default().push(updater);
    }
    for (deps, updaters) in grouped.iter() {
        let cond = deps
            .iter()
            .map(|d| format!("input.{0} !== currentInput.{0}", d))
            .join(" || ");
        update_lines.push(format!("if ({}) {{", cond));
        for updater in updaters {
            update_lines.push(format!("  {};", serialize_update(&updater.kind)));
        }
        update_lines.push("}".to_string());
    }

    // Add for loop update logic
    for (i, for_loop) in view.for_loops.iter().enumerate() {
        update_lines.push(format!(
            "childState{} = updateForLoop({{",
            for_loop.child_view_idx
        ));
        update_lines.push(format!("  anchor: anchor{},", i));
        update_lines.push(format!(
            "  prevStates: childState{},",
            for_loop.child_view_idx
        ));
        update_lines.push(format!(
            "  nextInputs: {seq}.map(({var}: any) => ({{ ...input, {var} }})),",
            seq = render_expr(&for_loop.sequence_expr),
            var = for_loop.var_name
        ));
        update_lines.push(format!("  subView: child{}", for_loop.child_view_idx));
        update_lines.push("});".to_string());
    }

    // Add use update logic
    for (i, use_info) in view.use_views.iter().enumerate() {
        let cond = use_info
            .view_dependencies
            .iter()
            .sorted()
            .map(|d| format!("input.{0} !== currentInput.{0}", d))
            .join(" || ");

        let input_obj = render_object(&use_info.input_attrs);

        update_lines.push(format!("if ({}) {{", cond));
        update_lines.push(format!(
            "  const newUseViewState{} = {}({});",
            i,
            render_expr(&use_info.view_expr),
            input_obj
        ));
        update_lines.push(format!(
            "  useViewState{}.root.replaceWith(newUseViewState{}.root);",
            i, i
        ));
        update_lines.push(format!("  useViewState{} = newUseViewState{};", i, i));
        update_lines.push("} else {".to_string());
        update_lines.push(format!("  useViewState{}.update({});", i, input_obj));
        update_lines.push("}".to_string());
    }

    // Add component call update logic
    for (i, component_call) in view.component_calls.iter().enumerate() {
        update_lines.push(format!(
            "componentState{}.update({});",
            i,
            render_object(&component_call.input_attrs)
        ));
    }

    // Add if update logic
    for (i, if_info) in view.ifs.iter().enumerate() {
        update_lines.push(format!(
            "if ({} !== {}) {{",
            render_expr(&if_info.condition_expr),
            render_expr_with_global_object(&if_info.condition_expr, "currentInput"),
        ));
        update_lines.push(format!("  let newState{}: ViewState<any>;", i));
        update_lines.push(format!(
            "  if ({}) {{",
            render_expr(&if_info.condition_expr)
        ));
        if let Some(then_idx) = if_info.then_view_idx {
            update_lines.push(format!("    newState{} = child{}(input);", i, then_idx));
        } else {
            update_lines.push(format!(
                    "    newState{} = {{ root: document.createComment(\"empty\"), update: (_: any) => {{}} }};",
                    i
                ));
        }
        update_lines.push("  } else {".to_string());
        if let Some(else_idx) = if_info.else_view_idx {
            update_lines.push(format!("    newState{} = child{}(input);", i, else_idx));
        } else {
            update_lines.push(format!(
                    "    newState{} = {{ root: document.createComment(\"empty\"), update: (_: any) => {{}} }};",
                    i
                ));
        }
        update_lines.push("  }".to_string());
        update_lines.push(format!("  const newRoot{} = newState{}.root;", i, i));
        update_lines.push(format!(
            "  currentState{}.root.replaceWith(newRoot{});",
            i, i
        ));
        update_lines.push(format!("  currentState{} = newState{};", i, i));
        update_lines.push("} else {".to_string());
        update_lines.push(format!("  currentState{}.update(input);", i));
        update_lines.push("}".to_string());
    }

    // Add switch update logic
    for (i, switch_info) in view.switches.iter().enumerate() {
        update_lines.push(format!(
            "const newOnValue{} = {}.type;",
            i,
            render_expr(&switch_info.on_expr)
        ));
        update_lines.push(format!(
            "const prevOnValue{} = {}.type;",
            i,
            render_expr_with_global_object(&switch_info.on_expr, "currentInput")
        ));
        update_lines.push(format!("if (newOnValue{} !== prevOnValue{}) {{", i, i));
        update_lines.push(format!("  let newState{}: ViewState<any>;", i));
        update_lines.push(format!("  let newRoot{}: any;", i));
        update_lines.push(format!("  switch (newOnValue{}) {{", i));
        for (j, case_name) in switch_info.case_names.iter().enumerate() {
            let case_idx = switch_info.case_view_idxs[j];
            update_lines.push(format!("    case \"{}\": {{", case_name));
            update_lines.push(format!(
                "      const caseInput = {{ ...input, {}: {} }};",
                case_name,
                render_expr(&switch_info.on_expr)
            ));
            update_lines.push(format!(
                "      newState{} = child{}(caseInput);",
                i, case_idx
            ));
            update_lines.push(format!("      newRoot{} = newState{}.root;", i, i));
            update_lines.push("      break;".to_string());
            update_lines.push("    }".to_string());
        }
        update_lines.push("    default: {".to_string());
        update_lines.push(format!("      newState{} = {{ root: document.createComment(\"switch-empty\"), update: (_: any) => {{}} }};", i));
        update_lines.push(format!("      newRoot{} = newState{}.root;", i, i));
        update_lines.push("    }".to_string());
        update_lines.push("  }".to_string());
        update_lines.push(format!(
            "  currentSwitchState{}.root.replaceWith(newRoot{});",
            i, i
        ));
        update_lines.push(format!("  currentSwitchState{} = newState{};", i, i));
        update_lines.push("} else {".to_string());
        update_lines.push(format!("  switch (newOnValue{}) {{", i));
        for case_name in switch_info.case_names.iter() {
            update_lines.push(format!("    case \"{}\": {{", case_name));
            update_lines.push(format!(
                "      const caseInput = {{ ...input, {}: {} }};",
                case_name,
                render_expr(&switch_info.on_expr)
            ));
            update_lines.push(format!("      currentSwitchState{}.update(caseInput);", i));
            update_lines.push("      break;".to_string());
            update_lines.push("    }".to_string());
        }
        update_lines.push("    default: {".to_string());
        update_lines.push("      // no-op".to_string());
        update_lines.push("    }".to_string());
        update_lines.push("  }".to_string());
        update_lines.push("}".to_string());
    }

    update_lines.push("currentInput = input;".to_string());

    // Apply indentation and assemble final output
    let indented_build_lines = apply_indent(&build_lines, indent, "  ");
    let indented_update_lines = apply_indent(&update_lines, indent, "      ");

    format!(
        "{build}\n{i}  return {{\n{i}    root,\n{i}    update(input) {{\n{update}\n{i}    }}\n  {i}}};",
        i = indent,
        build = indented_build_lines.join("\n"),
        update = indented_update_lines.join("\n"),
    )
}

pub fn emit_views(views: &[ViewDefinition]) -> String {
    let mut output = String::new();
    output.push_str("// Generated by VeGen. Do not edit.\n");

    // Sometimes we emit unused functions and variables, so disable type checking
    output.push_str("// @ts-nocheck\n\n");

    output.push_str(include_str!("lib.ts"));

    for view_def in views {
        let ViewDefinition {
            view_name,
            root,
            context,
            ts_type,
        } = view_def;
        let input_type_name = view_input_type_name(view_name);
        let type_str = ts_type.to_string();
        let input_type = format!("export type {} = {};\n", input_type_name, type_str);
        output.push_str(&input_type);
        let js_code = render(
            &CompiledView {
                root: root.clone(),
                context: context.clone(),
            },
            "",
        );
        output.push_str(&format!(
            "export function {}(input: {}): ViewState<{}> {{\n{}\n}}\n",
            view_name, input_type_name, input_type_name, js_code
        ));
    }
    output
}

pub fn render_object(obj: &BTreeMap<String, expr::Expr>) -> String {
    let fields = obj
        .iter()
        .map(|(k, v)| format!("{}: {}", render_key(k), render_expr(v)))
        .join(", ");
    format!("{{{}}}", fields)
}

pub fn render_expr(expr: &expr::Expr) -> String {
    render_expr_with_global_object(expr, "input")
}

fn render_expr_with_global_object(expr: &expr::Expr, global_object: &'static str) -> String {
    match expr {
        expr::Expr::Variable(name, _) => {
            if BUILTINS.contains_key(name) {
                return name.clone();
            }
            format!("{}.{}", global_object, name)
        }
        expr::Expr::Number(n, _) => n.clone(),
        expr::Expr::Field(f, field, _) => {
            format!(
                "{}.{}",
                render_expr_with_global_object(f, global_object),
                field
            )
        }
        expr::Expr::StringTemplate(segments, _) => {
            let mut result = String::new();
            for segment in segments {
                match segment {
                    expr::StringTemplateSegment::Literal(s) => result.push_str(s),
                    expr::StringTemplateSegment::Interpolation(e) => {
                        result.push_str(&format!(
                            "${{{}}}",
                            render_expr_with_global_object(e, global_object)
                        ));
                    }
                }
            }
            format!("`{}`", result)
        }
        expr::Expr::FunctionCall { callee, args, .. } => {
            let args_str = args
                .iter()
                .map(|e| render_expr_with_global_object(e, global_object))
                .join(", ");
            format!(
                "{}({})",
                render_expr_with_global_object(callee, global_object),
                args_str
            )
        }
        expr::Expr::Pipe { left, right, .. } => match right.as_ref() {
            expr::Expr::FunctionCall { callee, args, .. } => {
                let arg1_str = render_expr_with_global_object(left, global_object);
                let args_str = args
                    .iter()
                    .map(|e| render_expr_with_global_object(e, global_object))
                    .join(", ");
                format!(
                    "{}({}, {})",
                    render_expr_with_global_object(callee, global_object),
                    arg1_str,
                    args_str
                )
            }
            _ => {
                format!(
                    "{}({})",
                    render_expr_with_global_object(right, global_object),
                    render_expr_with_global_object(left, global_object)
                )
            }
        },
    }
}

fn render_attr_value(attr_value: &AttrValue) -> String {
    match attr_value {
        AttrValue::Template(segments) => {
            if segments.len() == 1 {
                if let StringTemplateSegment::Literal(s) = &segments[0] {
                    // Single literal: return as quoted string
                    format!("{:?}", s)
                } else {
                    let StringTemplateSegment::Interpolation(expr) = &segments[0] else {
                        unreachable!()
                    };
                    render_expr(expr)
                }
            } else {
                // Multiple segments: template literal
                let mut result = String::new();
                for segment in segments {
                    match segment {
                        StringTemplateSegment::Literal(s) => {
                            result.push_str(s);
                        }
                        StringTemplateSegment::Interpolation(expr) => {
                            result.push_str(&format!("${{{}}}", render_expr(expr)));
                        }
                    }
                }
                format!("`{}`", result)
            }
        }
        AttrValue::Expr(expr) => render_expr(expr),
    }
}

pub fn view_input_type_name(view_name: &str) -> String {
    format!("{}Input", view_name)
}
