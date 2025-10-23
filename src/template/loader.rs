use crate::error::Error;
use crate::graph::{cycle_from_stack, topo_sort};
use crate::lang::{parse_template, Span};
use crate::template::module::{TemplateModule, ViewStub};
use crate::template::path::normalize_path;
use crate::template::resolver::{io_to_error, resolve_required_path, TemplateResolver};
use crate::template::source_map::{SourceMap, TemplatePath};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

struct State {
    modules: HashMap<PathBuf, TemplateModule>,
    requires_graph: HashMap<PathBuf, Vec<PathBuf>>,
    visiting: HashSet<PathBuf>,
    stack: Vec<PathBuf>,
}

pub fn load_ordered_views<R: TemplateResolver>(
    entry_path: TemplatePath,
    resolver: &mut R,
    sources: &mut SourceMap,
) -> Result<Vec<ViewStub>, Vec<Error>> {
    let mut state = State {
        modules: HashMap::new(),
        requires_graph: HashMap::new(),
        visiting: HashSet::new(),
        stack: Vec::new(),
    };

    let normalized_entry = normalize_path(entry_path.as_ref().clone());
    visit(
        normalized_entry.clone(),
        None,
        resolver,
        &mut state,
        sources,
    )?;

    let modules_vec: Vec<TemplateModule> = state.modules.into_values().collect();

    let mut view_lookup: HashMap<String, (usize, usize)> = HashMap::new();
    let mut view_spans: HashMap<String, Span> = HashMap::new();
    for (module_idx, module) in modules_vec.iter().enumerate() {
        for (view_idx, view) in module.views.iter().enumerate() {
            if let Some((prev_module_idx, prev_view_idx)) = view_lookup.get(&view.name) {
                let previous = &modules_vec[*prev_module_idx].views[*prev_view_idx];
                return Err(Error {
                    message: format!(
                        "View '{}' is defined more than once (in '{}' and '{}').",
                        view.name,
                        modules_vec[*prev_module_idx].path.display(),
                        module.path.display()
                    ),
                    main_span: view.name_span,
                    labels: vec![
                        (view.name_span, "Second definition occurs here.".to_string()),
                        (previous.name_span, "First definition was here.".to_string()),
                    ],
                }
                .into());
            }
            view_lookup.insert(view.name.clone(), (module_idx, view_idx));
            view_spans.insert(view.name.clone(), view.name_span);
        }
    }

    let mut view_dependencies: HashMap<String, HashSet<String>> = HashMap::new();
    for module in modules_vec.iter() {
        let module_path = module.path.as_ref().clone();
        for view in &module.views {
            let mut deps = HashSet::new();
            for component in &view.component_refs {
                let Some((target_module_idx, _)) = view_lookup.get(&component.name) else {
                    return Err(Error {
                        message: format!(
                            "Component '{}' is not defined in this compilation set.",
                            component.name
                        ),
                        main_span: component.span,
                        labels: vec![(
                            component.span,
                            "Add a matching <view> definition or correct the name.".to_string(),
                        )],
                    }
                    .into());
                };

                let target_module = &modules_vec[*target_module_idx];
                let target_path = target_module.path.as_ref().clone();
                let direct_children = state.requires_graph.get(&module_path);
                let has_direct_path = module_path == target_path
                    || direct_children
                        .map(|children| children.contains(&target_path))
                        .unwrap_or(false);
                if !has_direct_path {
                    return Err(Error {
                        message: format!(
                            "Component '{}' is defined in '{}', but this template does not <require> it directly.",
                            component.name,
                            target_module.path.display()
                        ),
                        main_span: component.span,
                        labels: vec![(
                            component.span,
                            "Add or fix a <require src=\"…\"> directive.".to_string(),
                        )],
                    }
                    .into());
                }

                deps.insert(component.name.clone());
            }
            view_dependencies.insert(view.name.clone(), deps);
        }
    }

    let order = topologically_sort_views(&view_dependencies, |name| view_spans.get(name).cloned())?;

    let mut ordered_views = Vec::with_capacity(order.len());
    for view_name in order {
        if let Some((module_idx, view_idx)) = view_lookup.get(&view_name) {
            ordered_views.push(modules_vec[*module_idx].views[*view_idx].clone());
        }
    }

    Ok(ordered_views)
}

fn visit<R: TemplateResolver>(
    path: PathBuf,
    trigger_span: Option<Span>,
    resolver: &mut R,
    state: &mut State,
    sources: &mut SourceMap,
) -> Result<(), Vec<Error>> {
    if state.modules.contains_key(&path) {
        return Ok(());
    }
    if state.visiting.contains(&path) {
        let cycle = cycle_from_stack(&state.stack, &path);

        let mut message = "Circular <require> dependency detected: ".to_string();
        for (idx, segment) in cycle.iter().enumerate() {
            if idx > 0 {
                message.push_str(" -> ");
            }
            write!(message, "{}", segment.display()).unwrap();
        }

        let span = trigger_span.unwrap_or(Span {
            start: 0,
            end: 0,
            context: 0,
        });
        return Err(Error {
            message,
            main_span: span,
            labels: vec![(span, "Cycle introduced here.".to_string())],
        }
        .into());
    }

    state.visiting.insert(path.clone());
    state.stack.push(path.clone());

    let temp_path: TemplatePath = Arc::new(path.clone());
    let text = resolver
        .resolve(&temp_path)
        .map_err(|err| io_to_error(err, trigger_span, &path))?;

    let (source_id, template_path) = sources.ensure_entry(path.clone(), text.clone());

    let nodes = parse_template(&text, source_id)?;
    let module = TemplateModule::from_nodes(template_path, nodes)?;

    let mut resolved_children = Vec::new();
    let mut seen_child_paths = HashSet::new();

    for require in &module.requires {
        let resolved = normalize_path(resolve_required_path(&module.path, &require.raw_src));
        if !seen_child_paths.insert(resolved.clone()) {
            continue;
        }

        visit(
            resolved.clone(),
            Some(require.span),
            resolver,
            state,
            sources,
        )?;
        resolved_children.push(resolved);
    }

    state.requires_graph.insert(path.clone(), resolved_children);
    state.visiting.remove(&path);
    state.stack.pop();
    state.modules.insert(path, module);

    Ok(())
}

fn topologically_sort_views<F>(
    deps: &HashMap<String, HashSet<String>>,
    span_lookup: F,
) -> Result<Vec<String>, Error>
where
    F: Fn(&str) -> Option<Span>,
{
    match topo_sort(deps) {
        Ok(order) => Ok(order),
        Err(cycle) => {
            let mut message = "Circular component dependency: ".to_string();
            for (idx, segment) in cycle.nodes.iter().enumerate() {
                if idx > 0 {
                    message.push_str(" -> ");
                }
                message.push_str(segment);
            }

            let fallback_span = Span {
                start: 0,
                end: 0,
                context: 0,
            };

            let primary_name = cycle.nodes.last().cloned().unwrap_or_default();
            let span = span_lookup(&primary_name).unwrap_or(fallback_span);

            let mut labels = Vec::new();
            for node in &cycle.nodes {
                if let Some(node_span) = span_lookup(node) {
                    labels.push((node_span, format!("{} participates in the cycle.", node)));
                }
            }

            Err(Error {
                message,
                main_span: span,
                labels,
            })
        }
    }
}
