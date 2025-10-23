use crate::compile;
use crate::error::Error;
use crate::lsp::documents::{DocumentSnapshot, Documents};
use crate::template::{
    load_ordered_views, normalize_path, SourceMap, TemplatePath, TemplateResolver,
};
use lsp_types::{Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, Uri};
use std::{io, path::PathBuf, sync::Arc};
use url::Url;

const SOURCE: &str = "vegen";

pub fn collect(
    uri: &Uri,
    snapshot: &DocumentSnapshot<'_>,
    documents: &Documents,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(entry_path) = uri_to_normalized_path(uri) else {
        return diagnostics;
    };

    let entry_template_path: TemplatePath = Arc::new(entry_path.clone());
    let entry_text: Arc<str> = Arc::from(snapshot.text().to_string());

    let mut sources = SourceMap::new();
    sources.insert_with_id(
        entry_template_path.clone(),
        snapshot.source_id,
        entry_text.clone(),
    );

    let mut resolver = LspResolver {
        entry_path,
        entry_text,
        documents,
    };

    match load_ordered_views(entry_template_path, &mut resolver, &mut sources) {
        Ok(views) => {
            if let Err(error) = compile::compile_views(&views) {
                if let Some(diagnostic) = diagnostic_from_error(uri, snapshot, &error) {
                    diagnostics.push(diagnostic);
                }
            }
        }
        Err(errors) => {
            for error in errors {
                if let Some(diagnostic) = diagnostic_from_error(uri, snapshot, &error) {
                    diagnostics.push(diagnostic);
                }
            }
            return diagnostics;
        }
    }

    diagnostics.sort_by(|a, b| {
        (
            a.range.start.line,
            a.range.start.character,
            a.range.end.line,
            a.range.end.character,
        )
            .cmp(&(
                b.range.start.line,
                b.range.start.character,
                b.range.end.line,
                b.range.end.character,
            ))
    });

    diagnostics
}

fn diagnostic_from_error(
    uri: &Uri,
    snapshot: &DocumentSnapshot<'_>,
    error: &Error,
) -> Option<Diagnostic> {
    if error.main_span.context != snapshot.source_id {
        return None;
    }

    let range = snapshot.range_from_span(&error.main_span);
    let related_information = related_information(uri, snapshot, &error.labels);

    Some(Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some(SOURCE.to_string()),
        message: error.message.clone(),
        related_information,
        tags: None,
        data: None,
    })
}

fn related_information(
    uri: &Uri,
    snapshot: &DocumentSnapshot<'_>,
    labels: &[(crate::ast::Span, String)],
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let mut infos = Vec::new();

    for (span, message) in labels {
        if span.context != snapshot.source_id {
            continue;
        }

        let range = snapshot.range_from_span(span);
        infos.push(DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range,
            },
            message: message.clone(),
        });
    }

    if infos.is_empty() {
        None
    } else {
        Some(infos)
    }
}

pub(super) struct LspResolver<'a> {
    pub(super) entry_path: PathBuf,
    pub(super) entry_text: Arc<str>,
    pub(super) documents: &'a Documents,
}

impl TemplateResolver for LspResolver<'_> {
    fn resolve(&mut self, path: &TemplatePath) -> io::Result<Arc<str>> {
        let normalized = normalize_path(path.as_ref().to_path_buf());
        if normalized == self.entry_path {
            return Ok(self.entry_text.clone());
        }

        if let Some(snapshot) = self.documents.snapshot_by_path(&normalized) {
            return Ok(Arc::from(snapshot.text().to_string()));
        }

        let text = std::fs::read_to_string(path.as_ref())?;
        Ok(Arc::from(text))
    }
}

pub(super) fn uri_to_normalized_path(uri: &Uri) -> Option<PathBuf> {
    let url = Url::parse(&uri.to_string()).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok().map(normalize_path)
}
