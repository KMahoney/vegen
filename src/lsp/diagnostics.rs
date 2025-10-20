use crate::compile;
use crate::error::Error;
use crate::lsp::documents::DocumentSnapshot;
use crate::parser;
use lsp_types::{Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, Uri};

const SOURCE: &str = "vegen";

pub fn collect(uri: &Uri, snapshot: &DocumentSnapshot<'_>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    match parser::parse_template(snapshot.text(), snapshot.source_id) {
        Ok(nodes) => {
            if let Err(error) = compile::compile(&nodes) {
                if let Some(diagnostic) = diagnostic_from_error(uri, snapshot, &error) {
                    diagnostics.push(diagnostic);
                }
            }
        }
        Err(errors) => {
            for error in errors.iter() {
                if let Some(diagnostic) = diagnostic_from_error(uri, snapshot, error) {
                    diagnostics.push(diagnostic);
                }
            }
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
