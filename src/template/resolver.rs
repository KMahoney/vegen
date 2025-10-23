use crate::ast::Span;
use crate::error::Error;
use crate::template::source_map::TemplatePath;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

pub trait TemplateResolver {
    fn resolve(&mut self, path: &TemplatePath) -> io::Result<Arc<str>>;
}

pub fn io_to_error(err: io::Error, span: Option<Span>, path: &Path) -> Error {
    let main_span = span.unwrap_or(Span {
        start: 0,
        end: 0,
        context: 0,
    });

    Error {
        message: format!("Failed to load '{}': {}", path.display(), err),
        main_span,
        labels: vec![(main_span, "Unable to read required template.".to_string())],
    }
}

pub fn resolve_required_path(base_path: &TemplatePath, raw_src: &str) -> PathBuf {
    let raw = PathBuf::from(raw_src);
    if raw.is_absolute() || matches!(raw.components().next(), Some(Component::Prefix(_))) {
        raw
    } else {
        let base_dir = base_path
            .as_ref()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(PathBuf::new);
        base_dir.join(raw)
    }
}
