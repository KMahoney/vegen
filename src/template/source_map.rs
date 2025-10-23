use crate::lang::SourceId;
use crate::template::path::normalize_path;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub type TemplatePath = Arc<PathBuf>;

#[derive(Debug, Clone)]
pub struct SourceRecord {
    pub path: TemplatePath,
    pub text: Arc<str>,
}

#[derive(Debug, Default)]
pub struct SourceMap {
    path_to_id: HashMap<PathBuf, SourceId>,
    records: HashMap<SourceId, SourceRecord>,
    next_source_id: SourceId,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_with_id(
        &mut self,
        path: TemplatePath,
        source_id: SourceId,
        text: Arc<str>,
    ) -> SourceId {
        let normalized = normalize_path(path.as_ref().to_path_buf());
        self.path_to_id.insert(normalized, source_id);
        self.records.insert(
            source_id,
            SourceRecord {
                path: path.clone(),
                text,
            },
        );
        if self.next_source_id <= source_id {
            self.next_source_id = source_id + 1;
        }
        source_id
    }

    pub fn ensure_entry(&mut self, path: PathBuf, text: Arc<str>) -> (SourceId, TemplatePath) {
        if let Some(source_id) = self.path_to_id.get(&path).copied() {
            if let Some(record) = self.records.get_mut(&source_id) {
                record.text = text.clone();
                return (source_id, record.path.clone());
            }
        }

        let source_id = self.next_source_id;
        self.next_source_id += 1;

        let template_path: TemplatePath = Arc::new(path.clone());
        self.path_to_id.insert(path, source_id);
        self.records.insert(
            source_id,
            SourceRecord {
                path: template_path.clone(),
                text,
            },
        );

        (source_id, template_path)
    }

    pub fn record(&self, source_id: SourceId) -> Option<&SourceRecord> {
        self.records.get(&source_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (SourceId, &SourceRecord)> {
        let mut entries: Vec<(SourceId, &SourceRecord)> = self
            .records
            .iter()
            .map(|(id, record)| (*id, record))
            .collect();
        entries.sort_by_key(|(id, _)| *id);
        entries.into_iter()
    }
}
