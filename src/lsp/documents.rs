use crate::lang::{SourceId, Span};
use lsp_types::{Position, Range, TextDocumentContentChangeEvent, TextDocumentItem, Uri};
use std::{collections::HashMap, path::Path};
use url::Url;

#[derive(Debug, Clone)]
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut line_starts = Vec::new();
        line_starts.push(0);

        for (idx, ch) in text.char_indices() {
            if ch == '\n' {
                line_starts.push(idx + ch.len_utf8());
            }
        }

        Self { line_starts }
    }

    fn offset_to_position(&self, text: &str, offset: usize) -> Position {
        if self.line_starts.is_empty() {
            return Position::new(0, 0);
        }

        let capped_offset = offset.min(text.len());
        let line_index = match self.line_starts.binary_search(&capped_offset) {
            Ok(idx) => idx,
            Err(0) => 0,
            Err(idx) => idx - 1,
        };

        let line_start = self.line_starts[line_index];
        let slice = &text[line_start..capped_offset];
        let character = slice.encode_utf16().count() as u32;

        Position::new(line_index as u32, character)
    }

    pub fn range_from_span(&self, text: &str, span: &Span) -> Range {
        let start = self.offset_to_position(text, span.start);
        let end = self.offset_to_position(text, span.end);
        Range::new(start, end)
    }
}

pub struct DocumentState {
    pub source_id: SourceId,
    text: String,
    line_index: LineIndex,
    version: Option<i32>,
}

impl DocumentState {
    fn new(source_id: SourceId, version: Option<i32>, text: String) -> Self {
        let line_index = LineIndex::new(&text);
        Self {
            source_id,
            text,
            line_index,
            version,
        }
    }

    pub fn snapshot(&self) -> DocumentSnapshot<'_> {
        DocumentSnapshot {
            source_id: self.source_id,
            text: &self.text,
            line_index: &self.line_index,
            version: self.version,
        }
    }

    pub fn update(&mut self, version: Option<i32>, text: String) {
        self.text = text;
        self.version = version;
        self.line_index = LineIndex::new(&self.text);
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

pub struct DocumentSnapshot<'a> {
    pub source_id: SourceId,
    pub version: Option<i32>,
    text: &'a str,
    line_index: &'a LineIndex,
}

impl<'a> DocumentSnapshot<'a> {
    pub fn text(&self) -> &'a str {
        self.text
    }

    pub fn version(&self) -> Option<i32> {
        self.version
    }

    pub fn range_from_span(&self, span: &Span) -> Range {
        self.line_index.range_from_span(self.text, span)
    }
}

pub struct Documents {
    store: HashMap<Uri, DocumentState>,
    next_source_id: SourceId,
}

impl Documents {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
            next_source_id: 0,
        }
    }

    pub fn open(&mut self, item: TextDocumentItem) {
        let source_id = self.allocate_source_id();
        let version = Some(item.version);
        let TextDocumentItem { uri, text, .. } = item;
        let state = DocumentState::new(source_id, version, text);
        self.store.insert(uri, state);
    }

    pub fn update(
        &mut self,
        uri: &Uri,
        version: Option<i32>,
        changes: &[TextDocumentContentChangeEvent],
    ) -> Option<()> {
        let state = self.store.get_mut(uri)?;
        let new_text = changes
            .last()
            .map(|change| change.text.clone())
            .unwrap_or_else(|| state.text().to_string());
        state.update(version, new_text);
        Some(())
    }

    pub fn close(&mut self, uri: &Uri) -> Option<DocumentState> {
        self.store.remove(uri)
    }

    pub fn snapshot(&self, uri: &Uri) -> Option<DocumentSnapshot<'_>> {
        self.store.get(uri).map(DocumentState::snapshot)
    }

    pub fn snapshot_by_path(&self, path: &Path) -> Option<DocumentSnapshot<'_>> {
        let target = crate::template::normalize_path(path.to_path_buf());
        for (uri, state) in &self.store {
            let Ok(url) = Url::parse(&uri.to_string()) else {
                continue;
            };
            if url.scheme() != "file" {
                continue;
            }
            if let Ok(uri_path) = url.to_file_path() {
                if crate::template::normalize_path(uri_path) == target {
                    return Some(state.snapshot());
                }
            }
        }
        None
    }

    fn allocate_source_id(&mut self) -> SourceId {
        let id = self.next_source_id;
        self.next_source_id += 1;
        id
    }
}
