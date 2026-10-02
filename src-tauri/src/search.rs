use crate::persistence;
use serde::Deserialize;
use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tantivy::{
    collector::TopDocs,
    query::{BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, Query, RegexQuery, TermQuery},
    schema::{Field, IndexRecordOption, Schema, Value, STORED, STRING, TEXT},
    Index, IndexWriter, TantivyDocument, Term,
};
use uuid::Uuid;

const SEARCH_WRITER_HEAP_BYTES: usize = 50_000_000;
const OUTBOX_BATCH_SIZE: u32 = 2_000;
const MAX_SEARCH_RESULTS: u32 = 100;

#[derive(Debug, Clone)]
pub struct SearchState {
    root: PathBuf,
    gate: Arc<Mutex<()>>,
}

impl SearchState {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            gate: Arc::new(Mutex::new(())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResultRecord {
    pub document_id: String,
    pub file_instance_id: String,
    pub name: String,
    pub path: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_unix_ms: Option<i64>,
    pub availability_state: String,
    pub matched_field: String,
    pub score: f32,
}

#[derive(Debug)]
pub enum SearchError {
    Io(std::io::Error),
    Tantivy(tantivy::TantivyError),
    Persistence(persistence::PersistenceError),
    Json(serde_json::Error),
    Configuration(String),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Search I/O error: {error}"),
            Self::Tantivy(error) => write!(f, "Tantivy error: {error}"),
            Self::Persistence(error) => write!(f, "Search persistence error: {error}"),
            Self::Json(error) => write!(f, "Search outbox payload error: {error}"),
            Self::Configuration(message) => write!(f, "Search configuration error: {message}"),
        }
    }
}

impl Error for SearchError {}

impl From<std::io::Error> for SearchError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<tantivy::TantivyError> for SearchError {
    fn from(value: tantivy::TantivyError) -> Self {
        Self::Tantivy(value)
    }
}

impl From<persistence::PersistenceError> for SearchError {
    fn from(value: persistence::PersistenceError) -> Self {
        Self::Persistence(value)
    }
}

impl From<serde_json::Error> for SearchError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPayload {
    document_id: String,
    file_instance_id: String,
    content_version_id: Option<String>,
    storage_root_id: String,
    display_name: String,
    relative_path: String,
    size_bytes: u64,
    modified_unix_ms: Option<i64>,
    availability_state: String,
}

#[derive(Debug, Clone)]
struct SearchFields {
    document_id: Field,
    file_instance_id: Field,
    storage_root_id: Field,
    filename_exact: Field,
    filename_tokens: Field,
    path_tokens: Field,
    availability_state: Field,
}

impl SearchFields {
    fn from_schema(schema: &Schema) -> Result<Self, SearchError> {
        Ok(Self {
            document_id: schema_field(schema, "document_id")?,
            file_instance_id: schema_field(schema, "file_instance_id")?,
            storage_root_id: schema_field(schema, "storage_root_id")?,
            filename_exact: schema_field(schema, "filename_exact")?,
            filename_tokens: schema_field(schema, "filename_tokens")?,
            path_tokens: schema_field(schema, "path_tokens")?,
            availability_state: schema_field(schema, "availability_state")?,
        })
    }
}

pub fn sync_search_index(
    database_path: &Path,
    state: &SearchState,
) -> Result<(), SearchError> {
    let _guard = state
        .gate
        .lock()
        .map_err(|_| SearchError::Configuration("search-index lock was poisoned".to_string()))?;

    let index = ensure_index(database_path, &state.root)?;
    apply_pending_outbox(database_path, &index)?;
    Ok(())
}

pub fn rebuild_search_index(
    database_path: &Path,
    state: &SearchState,
) -> Result<(), SearchError> {
    let _guard = state
        .gate
        .lock()
        .map_err(|_| SearchError::Configuration("search-index lock was poisoned".to_string()))?;

    rebuild_index(database_path, &state.root)?;
    let index = open_active_index(&state.root)?;
    apply_pending_outbox(database_path, &index)?;
    Ok(())
}

pub fn search_documents(
    database_path: &Path,
    state: &SearchState,
    raw_query: &str,
    limit: u32,
) -> Result<Vec<SearchResultRecord>, SearchError> {
    let normalized_query = persistence::normalize_search_text(raw_query);
    if normalized_query.is_empty() {
        return Ok(Vec::new());
    }

    let query_tokens: Vec<String> = normalized_query
        .split_whitespace()
        .take(8)
        .map(ToOwned::to_owned)
        .collect();

    if query_tokens.is_empty() {
        return Ok(Vec::new());
    }

    let _guard = state
        .gate
        .lock()
        .map_err(|_| SearchError::Configuration("search-index lock was poisoned".to_string()))?;

    let index = ensure_index(database_path, &state.root)?;
    apply_pending_outbox(database_path, &index)?;

    let schema = index.schema();
    let fields = SearchFields::from_schema(&schema)?;
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let query = build_query(&fields, &normalized_query, &query_tokens)?;
    let requested_limit = limit.clamp(1, MAX_SEARCH_RESULTS) as usize;
    let candidate_limit = requested_limit.saturating_mul(3).min(300);
    let top_docs =
        searcher.search(&query, &TopDocs::with_limit(candidate_limit).order_by_score())?;

    let mut scored_ids = Vec::with_capacity(top_docs.len());

    for (score, address) in top_docs {
        let document = searcher.doc::<TantivyDocument>(address)?;
        let Some(value) = document.get_first(fields.file_instance_id) else {
            continue;
        };
        let Some(file_instance_id) = value.as_value().as_str() else {
            continue;
        };
        scored_ids.push((score, file_instance_id.to_string()));
    }

    let ids: Vec<String> = scored_ids
        .iter()
        .map(|(_, file_instance_id)| file_instance_id.clone())
        .collect();
    let hydrated = persistence::hydrate_search_files(database_path, &ids)?;

    let mut results = Vec::with_capacity(requested_limit);
    for ((score, _), file) in scored_ids.into_iter().zip(hydrated) {
        let Some(file) = file else {
            continue;
        };

        let matched_field =
            matched_field_label(&normalized_query, &query_tokens, &file.name, &file.path);

        results.push(SearchResultRecord {
            document_id: file.document_id,
            file_instance_id: file.file_instance_id,
            name: file.name,
            path: file.path,
            extension: file.extension,
            size_bytes: file.size_bytes,
            modified_unix_ms: file.modified_unix_ms,
            availability_state: file.availability_state,
            matched_field,
            score,
        });

        if results.len() >= requested_limit {
            break;
        }
    }

    Ok(results)
}

fn build_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("document_id", STRING | STORED);
    builder.add_text_field("file_instance_id", STRING | STORED);
    builder.add_text_field("storage_root_id", STRING);
    builder.add_text_field("filename_exact", STRING);
    builder.add_text_field("filename_tokens", TEXT);
    builder.add_text_field("path_tokens", TEXT);
    builder.add_text_field("availability_state", STRING);
    builder.build()
}

fn schema_field(schema: &Schema, name: &str) -> Result<Field, SearchError> {
    schema
        .get_field(name)
        .map_err(|_| SearchError::Configuration(format!("search index is missing field {name}")))
}

fn ensure_index(database_path: &Path, root: &Path) -> Result<Index, SearchError> {
    fs::create_dir_all(root)?;
    recover_interrupted_swap(root)?;

    let active = active_index_path(root);
    if active.exists() {
        if let Ok(index) = Index::open_in_dir(&active) {
            if SearchFields::from_schema(&index.schema()).is_ok() {
                return Ok(index);
            }
        }
    }

    rebuild_index(database_path, root)?;
    open_active_index(root)
}

fn rebuild_index(database_path: &Path, root: &Path) -> Result<(), SearchError> {
    fs::create_dir_all(root)?;
    recover_interrupted_swap(root)?;

    let pending_before_snapshot = persistence::list_pending_search_outbox_ids(database_path)?;
    let snapshot = persistence::list_search_projection_snapshot(database_path)?;
    let building = root.join(format!("building-{}", Uuid::new_v4()));
    fs::create_dir_all(&building)?;

    let schema = build_schema();
    let index = Index::create_in_dir(&building, schema)?;
    let fields = SearchFields::from_schema(&index.schema())?;
    let mut writer = index.writer(SEARCH_WRITER_HEAP_BYTES)?;

    for record in snapshot {
        add_projection_document(&mut writer, &fields, &record)?;
    }

    writer.commit()?;
    drop(writer);
    drop(index);

    swap_in_built_index(root, &building)?;
    persistence::acknowledge_search_outbox(database_path, &pending_before_snapshot)?;
    Ok(())
}

fn apply_pending_outbox(database_path: &Path, index: &Index) -> Result<(), SearchError> {
    let fields = SearchFields::from_schema(&index.schema())?;

    loop {
        let batch = persistence::list_pending_search_outbox(database_path, OUTBOX_BATCH_SIZE)?;
        if batch.is_empty() {
            return Ok(());
        }

        let operation_ids: Vec<String> =
            batch.iter().map(|record| record.operation_id.clone()).collect();

        let result = apply_outbox_batch(index, &fields, &batch);
        if let Err(error) = result {
            let _ = persistence::record_search_outbox_failure(
                database_path,
                &operation_ids,
                &error.to_string(),
            );
            return Err(error);
        }

        persistence::acknowledge_search_outbox(database_path, &operation_ids)?;

        if batch.len() < OUTBOX_BATCH_SIZE as usize {
            return Ok(());
        }
    }
}

fn apply_outbox_batch(
    index: &Index,
    fields: &SearchFields,
    batch: &[persistence::SearchOutboxRecord],
) -> Result<(), SearchError> {
    let mut writer = index.writer(SEARCH_WRITER_HEAP_BYTES)?;

    for record in batch {
        match record.operation.as_str() {
            "UPSERT" => {
                if record.payload_version != 1 {
                    return Err(SearchError::Configuration(format!(
                        "unsupported search payload version {}",
                        record.payload_version
                    )));
                }

                let payload: SearchPayload = serde_json::from_str(&record.payload_json)?;
                writer.delete_term(Term::from_field_text(
                    fields.file_instance_id,
                    &payload.file_instance_id,
                ));
                add_payload_document(&mut writer, fields, &payload)?;
            }
            "DELETE" => {
                writer.delete_term(Term::from_field_text(
                    fields.document_id,
                    &record.entity_id,
                ));
            }
            other => {
                return Err(SearchError::Configuration(format!(
                    "unsupported search outbox operation {other}"
                )));
            }
        }
    }

    writer.commit()?;
    Ok(())
}

fn add_projection_document(
    writer: &mut IndexWriter,
    fields: &SearchFields,
    record: &persistence::SearchProjectionRecord,
) -> Result<(), SearchError> {
    let document = build_document(
        fields,
        &record.document_id,
        &record.file_instance_id,
        &record.storage_root_id,
        &record.display_name,
        &record.relative_path_display,
        &record.availability_state,
    );
    writer.add_document(document)?;
    Ok(())
}

fn add_payload_document(
    writer: &mut IndexWriter,
    fields: &SearchFields,
    payload: &SearchPayload,
) -> Result<(), SearchError> {
    let _ = (
        payload.content_version_id.as_deref(),
        payload.size_bytes,
        payload.modified_unix_ms,
    );

    let document = build_document(
        fields,
        &payload.document_id,
        &payload.file_instance_id,
        &payload.storage_root_id,
        &payload.display_name,
        &payload.relative_path,
        &payload.availability_state,
    );
    writer.add_document(document)?;
    Ok(())
}

fn build_document(
    fields: &SearchFields,
    document_id: &str,
    file_instance_id: &str,
    storage_root_id: &str,
    display_name: &str,
    relative_path: &str,
    availability_state: &str,
) -> TantivyDocument {
    let filename_normalized = persistence::normalize_search_text(display_name);
    let filename_stem = Path::new(display_name)
        .file_stem()
        .map(|value| persistence::normalize_search_text(&value.to_string_lossy()))
        .unwrap_or_else(|| filename_normalized.clone());
    let path_normalized = persistence::normalize_search_text(relative_path);

    let mut document = TantivyDocument::default();
    document.add_text(fields.document_id, document_id);
    document.add_text(fields.file_instance_id, file_instance_id);
    document.add_text(fields.storage_root_id, storage_root_id);
    document.add_text(fields.filename_exact, &filename_normalized);
    if filename_stem != filename_normalized && !filename_stem.is_empty() {
        document.add_text(fields.filename_exact, filename_stem);
    }
    document.add_text(fields.filename_tokens, filename_normalized);
    document.add_text(fields.path_tokens, path_normalized);
    document.add_text(
        fields.availability_state,
        availability_state.to_ascii_lowercase(),
    );
    document
}

fn build_query(
    fields: &SearchFields,
    normalized_query: &str,
    tokens: &[String],
) -> Result<Box<dyn Query>, SearchError> {
    let mut outer: Vec<(Occur, Box<dyn Query>)> = Vec::new();

    for token in tokens {
        let mut alternatives: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        alternatives.push((
            Occur::Should,
            boosted_term(fields.filename_tokens, token, 12.0),
        ));

        if token.len() >= 2 {
            alternatives.push((
                Occur::Should,
                boosted_regex(fields.filename_tokens, &format!("^{}.*", escape_regex(token)), 10.0)?,
            ));
        }

        if token.len() >= 3 {
            alternatives.push((
                Occur::Should,
                boosted_regex(
                    fields.filename_tokens,
                    &format!(".*{}.*", escape_regex(token)),
                    7.0,
                )?,
            ));
        }

        if token.len() >= 4 {
            let distance = if token.len() >= 8 { 2 } else { 1 };
            let fuzzy = FuzzyTermQuery::new_prefix(
                Term::from_field_text(fields.filename_tokens, token),
                distance,
                true,
            );
            alternatives.push((
                Occur::Should,
                Box::new(BoostQuery::new(Box::new(fuzzy), 5.0)),
            ));
        }

        alternatives.push((
            Occur::Should,
            boosted_term(fields.path_tokens, token, 3.0),
        ));

        if token.len() >= 2 {
            alternatives.push((
                Occur::Should,
                boosted_regex(fields.path_tokens, &format!("^{}.*", escape_regex(token)), 2.5)?,
            ));
        }

        if token.len() >= 4 {
            alternatives.push((
                Occur::Should,
                boosted_regex(
                    fields.path_tokens,
                    &format!(".*{}.*", escape_regex(token)),
                    1.5,
                )?,
            ));
        }

        outer.push((
            Occur::Must,
            Box::new(BooleanQuery::with_minimum_required_clauses(alternatives, 1)),
        ));
    }

    let exact_filename = TermQuery::new(
        Term::from_field_text(fields.filename_exact, normalized_query),
        IndexRecordOption::Basic,
    );
    outer.push((
        Occur::Should,
        Box::new(BoostQuery::new(Box::new(exact_filename), 30.0)),
    ));

    Ok(Box::new(BooleanQuery::new(outer)))
}

fn boosted_term(field: Field, token: &str, boost: f32) -> Box<dyn Query> {
    let query = TermQuery::new(
        Term::from_field_text(field, token),
        IndexRecordOption::WithFreqs,
    );
    Box::new(BoostQuery::new(Box::new(query), boost))
}

fn boosted_regex(field: Field, pattern: &str, boost: f32) -> Result<Box<dyn Query>, SearchError> {
    let query = RegexQuery::from_pattern(pattern, field)?;
    Ok(Box::new(BoostQuery::new(Box::new(query), boost)))
}

fn matched_field_label(
    normalized_query: &str,
    tokens: &[String],
    filename: &str,
    path: &str,
) -> String {
    let normalized_filename = persistence::normalize_search_text(filename);
    let normalized_stem = Path::new(filename)
        .file_stem()
        .map(|value| persistence::normalize_search_text(&value.to_string_lossy()))
        .unwrap_or_default();

    if normalized_filename == normalized_query || normalized_stem == normalized_query {
        return "Filename exact".to_string();
    }

    let filename_terms: Vec<&str> = normalized_filename.split_whitespace().collect();
    let filename_matches = tokens.iter().all(|token| {
        filename_terms
            .iter()
            .any(|term| term == token || term.starts_with(token) || term.contains(token))
    });

    if filename_matches {
        return "Filename".to_string();
    }

    let normalized_path = persistence::normalize_search_text(path);
    if tokens.iter().all(|token| normalized_path.contains(token)) {
        return "Path".to_string();
    }

    "Filename fuzzy".to_string()
}

fn escape_regex(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for character in value.chars() {
        if matches!(
            character,
            '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }

    escaped
}

fn active_index_path(root: &Path) -> PathBuf {
    root.join("active")
}

fn previous_index_path(root: &Path) -> PathBuf {
    root.join("previous")
}

fn open_active_index(root: &Path) -> Result<Index, SearchError> {
    Ok(Index::open_in_dir(active_index_path(root))?)
}

fn recover_interrupted_swap(root: &Path) -> Result<(), SearchError> {
    let active = active_index_path(root);
    let previous = previous_index_path(root);

    if !active.exists() && previous.exists() {
        fs::rename(&previous, &active)?;
    } else if active.exists() && previous.exists() {
        fs::remove_dir_all(&previous)?;
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("building-") && entry.path().is_dir() {
            fs::remove_dir_all(entry.path())?;
        }
    }

    Ok(())
}

fn swap_in_built_index(root: &Path, building: &Path) -> Result<(), SearchError> {
    let active = active_index_path(root);
    let previous = previous_index_path(root);

    if previous.exists() {
        fs::remove_dir_all(&previous)?;
    }

    if active.exists() {
        fs::rename(&active, &previous)?;
    }

    if let Err(error) = fs::rename(building, &active) {
        if !active.exists() && previous.exists() {
            let _ = fs::rename(&previous, &active);
        }
        return Err(SearchError::Io(error));
    }

    if previous.exists() {
        fs::remove_dir_all(previous)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer;
    use std::sync::atomic::AtomicBool;

    struct TestSearch {
        directory: PathBuf,
        database_path: PathBuf,
        source_root: PathBuf,
        search_state: SearchState,
    }

    impl TestSearch {
        fn new() -> Self {
            let directory =
                std::env::temp_dir().join(format!("professional-docx-search-{}", Uuid::new_v4()));
            let database_path = directory.join("data").join("metadata.sqlite");
            let source_root = directory.join("source");
            let search_state = SearchState::new(directory.join("data").join("search-index"));

            fs::create_dir_all(&source_root).expect("test source root should be created");
            persistence::initialize_database(&database_path)
                .expect("test database should initialize");

            Self {
                directory,
                database_path,
                source_root,
                search_state,
            }
        }

        fn register_root(&self, root_id: &str) -> persistence::StorageRootRecord {
            let canonical =
                fs::canonicalize(&self.source_root).expect("source root should canonicalize");
            persistence::register_storage_root(
                &self.database_path,
                root_id,
                &self.source_root,
                &canonical,
            )
            .expect("source root should register")
        }

        fn scan(&self, root: &persistence::StorageRootRecord) {
            let job_id = Uuid::new_v4().to_string();
            let generation_id = Uuid::new_v4().to_string();

            persistence::create_index_job(
                &self.database_path,
                &root.storage_root_id,
                &job_id,
                &generation_id,
            )
            .expect("index job should be created");

            indexer::run_index_job(
                self.database_path.clone(),
                root.clone(),
                job_id,
                generation_id,
                Arc::new(AtomicBool::new(false)),
            )
            .expect("index job should complete");
        }
    }

    impl Drop for TestSearch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn universal_search_handles_exact_partial_typo_and_period_normalization() {
        let test = TestSearch::new();
        fs::write(
            test.source_root.join("2024-25 Salamudd ITR.pdf"),
            b"tax return",
        )
        .expect("target file should be written");
        fs::write(test.source_root.join("Salamudd Notes.txt"), b"notes")
            .expect("secondary file should be written");

        let root = test.register_root("root-search");
        test.scan(&root);
        sync_search_index(&test.database_path, &test.search_state)
            .expect("search index should sync");

        let cases = [
            "2024-25 Salamudd ITR",
            "salamudd",
            "salamud",
            "lamud",
            "salammudd",
            "2024 salamudd",
            "2024_25 salamudd",
            "2024.25 salamudd",
        ];

        for query in cases {
            let results = search_documents(
                &test.database_path,
                &test.search_state,
                query,
                10,
            )
            .expect("search should succeed");

            assert!(
                results
                    .iter()
                    .any(|result| result.name == "2024-25 Salamudd ITR.pdf"),
                "query {query:?} should find the target file"
            );
        }

        let exact = search_documents(
            &test.database_path,
            &test.search_state,
            "2024-25 Salamudd ITR",
            10,
        )
        .expect("exact title search should succeed");
        assert_eq!(exact[0].name, "2024-25 Salamudd ITR.pdf");
        assert_eq!(exact[0].matched_field, "Filename exact");

        let pending = persistence::list_pending_search_outbox_ids(&test.database_path)
            .expect("pending outbox ids should load");
        assert!(pending.is_empty());
    }

    #[test]
    fn deleted_tantivy_index_is_rebuilt_from_sqlite() {
        let test = TestSearch::new();
        fs::write(test.source_root.join("RCM March.xlsx"), b"rcm")
            .expect("test file should be written");

        let root = test.register_root("root-rebuild");
        test.scan(&root);
        sync_search_index(&test.database_path, &test.search_state)
            .expect("search index should sync");

        fs::remove_dir_all(active_index_path(&test.search_state.root))
            .expect("active Tantivy index should be removable");

        let results =
            search_documents(&test.database_path, &test.search_state, "rcm march", 10)
                .expect("search should rebuild from SQLite");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "RCM March.xlsx");
        assert!(active_index_path(&test.search_state.root).exists());
    }

    #[test]
    fn missing_source_remains_searchable_after_authoritative_reconciliation() {
        let test = TestSearch::new();
        let path = test.source_root.join("Bank Confirmation.pdf");
        fs::write(&path, b"confirmation").expect("test file should be written");

        let root = test.register_root("root-missing-search");
        test.scan(&root);
        sync_search_index(&test.database_path, &test.search_state)
            .expect("initial search index should sync");

        fs::remove_file(path).expect("source file should be removed");
        test.scan(&root);
        sync_search_index(&test.database_path, &test.search_state)
            .expect("reconciled search index should sync");

        let results =
            search_documents(&test.database_path, &test.search_state, "bank confirmation", 10)
                .expect("missing file should remain searchable");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].availability_state, "MISSING");
    }
}
