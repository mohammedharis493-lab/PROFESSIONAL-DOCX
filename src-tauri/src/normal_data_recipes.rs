//! Immutable Normal Data comparison recipe definitions.
//! Source bytes and result rows are never accepted from frontend commands.
//! A recipe documents the configured dataset identities and declared roles;
//! it does not assert that linked bytes remain available or hash-verified.
use crate::persistence::{self, PersistenceError};
use rusqlite::{params, TransactionBehavior};
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeRecord {
    pub normal_data_comparison_recipe_id: String,
    pub normal_data_comparison_recipe_version_id: String,
    pub normal_data_workspace_id: String,
    pub version_number: i64,
    pub name: String,
    pub dataset_a_version_id: String,
    pub dataset_b_version_id: String,
    pub period_basis: String,
    pub amount_columns: Vec<String>,
    pub tolerance_minor_units: i64,
    pub created_at_ms: i64,
}

pub struct RecipeDefinition<'a> {
    pub normal_data_workspace_id: &'a str,
    pub name: &'a str,
    pub dataset_a_version_id: &'a str,
    pub dataset_b_version_id: &'a str,
    pub period_basis: &'a str,
    pub amount_columns: &'a [String],
    pub tolerance_minor_units: i64,
}

fn invalid(message: impl Into<String>) -> PersistenceError {
    PersistenceError::Configuration(message.into())
}

fn validate_definition(input: &RecipeDefinition<'_>)
    -> Result<(String, Vec<String>, &'static str, &'static str), PersistenceError>
{
    let name = input.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > 240 || name.chars().any(char::is_control) {
        return Err(invalid("comparison recipe name must be 1 to 240 printable characters"));
    }
    if input.dataset_a_version_id == input.dataset_b_version_id {
        return Err(invalid("comparison requires two different dataset versions"));
    }
    if input.tolerance_minor_units < 0 {
        return Err(invalid("comparison tolerance cannot be negative"));
    }
    let (period_role, period_type) = match input.period_basis {
        "FILING_PERIOD" => ("FILING_PERIOD", "PERIOD"),
        "INVOICE_MONTH" => ("INVOICE_DATE", "DATE"),
        "ACCOUNTING_PERIOD" => ("ACCOUNTING_PERIOD", "PERIOD"),
        _ => return Err(invalid("unsupported comparison period basis")),
    };
    if input.amount_columns.is_empty() || input.amount_columns.len() > 32 {
        return Err(invalid("comparison needs 1 to 32 numeric fields"));
    }
    let mut columns = Vec::with_capacity(input.amount_columns.len());
    let mut names = BTreeSet::new();
    for value in input.amount_columns {
        let name = value.trim();
        if name.is_empty() || name.chars().count() > 240
            || name != value || name.chars().any(char::is_control)
            || !names.insert(name.to_lowercase())
        {
            return Err(invalid("comparison numeric columns must be distinct and printable"));
        }
        columns.push(name.to_string());
    }
    columns.sort();
    Ok((name, columns, period_role, period_type))
}

pub fn create_recipe(
    database_path: &Path,
    input: RecipeDefinition<'_>,
) -> Result<RecipeRecord, PersistenceError> {
    let (name, columns, period_role, period_type) = validate_definition(&input)?;
    let mut connection = persistence::open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let workspace_exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM normal_data_workspaces WHERE normal_data_workspace_id = ?1)",
        [input.normal_data_workspace_id],
        |row| row.get(0),
    )?;
    if !workspace_exists {
        return Err(invalid("normal data workspace does not exist"));
    }
    for dataset_version_id in [input.dataset_a_version_id, input.dataset_b_version_id] {
        let within_workspace: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM normal_data_dataset_versions v
                JOIN normal_data_datasets d ON d.normal_data_dataset_id = v.normal_data_dataset_id
                WHERE v.normal_data_dataset_version_id = ?1
                  AND d.normal_data_workspace_id = ?2
             )",
            params![dataset_version_id, input.normal_data_workspace_id],
            |row| row.get(0),
        )?;
        if !within_workspace {
            return Err(invalid("both dataset versions must belong to this workspace"));
        }
        let selected_period_role: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM normal_data_column_semantics
                WHERE normal_data_dataset_version_id = ?1
                  AND semantic_role = ?2 AND data_type = ?3
             )",
            params![dataset_version_id, period_role, period_type],
            |row| row.get(0),
        )?;
        if !selected_period_role {
            return Err(invalid("selected period role is not declared on both dataset versions"));
        }
        for column in &columns {
            let declared_numeric: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM normal_data_column_semantics
                    WHERE normal_data_dataset_version_id = ?1
                      AND column_name = ?2 COLLATE NOCASE
                      AND semantic_role = 'NUMERIC_VALUE' AND data_type = 'DECIMAL'
                )",
                params![dataset_version_id, column],
                |row| row.get(0),
            )?;
            if !declared_numeric {
                return Err(invalid(format!("numeric field {column} is not declared on both datasets")));
            }
        }
    }
    let recipe_id = Uuid::new_v4().to_string();
    let version_id = Uuid::new_v4().to_string();
    let now = persistence::now_unix_ms()?;
    let amount_columns_json = serde_json::to_string(&columns)
        .map_err(|error| invalid(format!("cannot encode numeric columns: {error}")))?;
    transaction.execute(
        "INSERT INTO normal_data_comparison_recipes (
            normal_data_comparison_recipe_id, normal_data_workspace_id, created_at_ms
         ) VALUES (?1, ?2, ?3)",
        params![&recipe_id, input.normal_data_workspace_id, now],
    )?;
    transaction.execute(
        "INSERT INTO normal_data_comparison_recipe_versions (
            normal_data_comparison_recipe_version_id, normal_data_comparison_recipe_id,
            version_number, name, dataset_a_version_id, dataset_b_version_id,
            period_basis, amount_columns_json, tolerance_minor_units, created_at_ms
         ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            &version_id, &recipe_id, &name, input.dataset_a_version_id,
            input.dataset_b_version_id, input.period_basis, &amount_columns_json,
            input.tolerance_minor_units, now,
        ],
    )?;
    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id, event_type, entity_type, entity_id,
            related_entity_type, related_entity_id, occurred_at_ms,
            actor_id, details_json
         ) VALUES (?1, 'NORMAL_DATA_RECIPE_CREATED', 'NORMAL_DATA_COMPARISON_RECIPE',
                   ?2, 'NORMAL_DATA_WORKSPACE', ?3, ?4, NULL, ?5)",
        params![
            Uuid::new_v4().to_string(), &recipe_id, input.normal_data_workspace_id, now,
            json!({"recipe_version_id":version_id,"period_basis":input.period_basis,
                   "dataset_a_version_id":input.dataset_a_version_id,
                   "dataset_b_version_id":input.dataset_b_version_id}).to_string(),
        ],
    )?;
    transaction.commit()?;
    Ok(RecipeRecord {
        normal_data_comparison_recipe_id: recipe_id,
        normal_data_comparison_recipe_version_id: version_id,
        normal_data_workspace_id: input.normal_data_workspace_id.to_string(),
        version_number: 1,
        name,
        dataset_a_version_id: input.dataset_a_version_id.to_string(),
        dataset_b_version_id: input.dataset_b_version_id.to_string(),
        period_basis: input.period_basis.to_string(),
        amount_columns: columns,
        tolerance_minor_units: input.tolerance_minor_units,
        created_at_ms: now,
    })
}

pub fn list_recipes(
    database_path: &Path,
    workspace_id: &str,
) -> Result<Vec<RecipeRecord>, PersistenceError> {
    let connection = persistence::open_configured_connection(database_path)?;
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM normal_data_workspaces WHERE normal_data_workspace_id = ?1)",
        [workspace_id], |row| row.get(0),
    )?;
    if !exists {
        return Err(invalid("normal data workspace does not exist"));
    }
    let mut statement = connection.prepare(
        "SELECT r.normal_data_comparison_recipe_id, v.normal_data_comparison_recipe_version_id,
                r.normal_data_workspace_id, v.version_number, v.name,
                v.dataset_a_version_id, v.dataset_b_version_id, v.period_basis,
                v.amount_columns_json, v.tolerance_minor_units, v.created_at_ms
         FROM normal_data_comparison_recipes r
         JOIN normal_data_comparison_recipe_versions v
           ON v.normal_data_comparison_recipe_id = r.normal_data_comparison_recipe_id
          AND v.version_number = (
              SELECT MAX(v2.version_number) FROM normal_data_comparison_recipe_versions v2
              WHERE v2.normal_data_comparison_recipe_id = r.normal_data_comparison_recipe_id
          )
         WHERE r.normal_data_workspace_id = ?1
         ORDER BY r.created_at_ms DESC, r.normal_data_comparison_recipe_id",
    )?;
    let rows = statement.query_map([workspace_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, i64>(9)?,
            row.get::<_, i64>(10)?,
        ))
    })?;
    let mut result = Vec::new();
    for row in rows {
        let (recipe_id, version_id, workspace_id, version_number, name,
            dataset_a_version_id, dataset_b_version_id, period_basis,
            amount_columns_json, tolerance_minor_units, created_at_ms) = row?;
        let amount_columns = serde_json::from_str::<Vec<String>>(&amount_columns_json)
            .map_err(|error| invalid(format!("stored numeric columns are invalid: {error}")))?;
        result.push(RecipeRecord {
            normal_data_comparison_recipe_id: recipe_id,
            normal_data_comparison_recipe_version_id: version_id,
            normal_data_workspace_id: workspace_id,
            version_number,
            name,
            dataset_a_version_id,
            dataset_b_version_id,
            period_basis,
            amount_columns,
            tolerance_minor_units,
            created_at_ms,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_recipe_is_workspace_scoped_audited_and_immutable() {
        use crate::normal_data::{self, WorkspaceDefinition};
        use crate::normal_data_datasets;
        use rusqlite::params;
        use std::fs;

        let folder = std::env::temp_dir().join(format!("pdox-recipe-{}", Uuid::new_v4()));
        fs::create_dir_all(&folder).expect("fixture parent");
        let database_path = folder.join("metadata.sqlite");
        persistence::initialize_database(&database_path).expect("initialize");
        let workspace = normal_data::create_workspace(
            &database_path,
            WorkspaceDefinition {
                name: "Normal data", description: None, client_id: None,
                period_start: None, period_end: None,
            },
        ).expect("create engagement-independent workspace");
        let other_workspace = normal_data::create_workspace(
            &database_path,
            WorkspaceDefinition {
                name: "Unrelated normal data", description: None, client_id: None,
                period_start: None, period_end: None,
            },
        ).expect("create unrelated workspace");
        let root_id = Uuid::new_v4().to_string();
        let connection = persistence::open_configured_connection(&database_path)
            .expect("open database");
        connection.execute(
            "INSERT INTO storage_roots (
                storage_root_id, kind, native_locator, native_locator_encoding,
                display_locator, availability_state, approved_at_ms,
                created_at_ms, updated_at_ms
            ) VALUES (?1, 'LOCAL', X'00', 'test', 'fixture only',
                      'AVAILABLE', 1, 1, 1)",
            [&root_id],
        ).expect("insert synthetic source identity");
        let mut versions = Vec::new();
        for index in 0..2 {
            let document_id = Uuid::new_v4().to_string();
            let file_instance_id = Uuid::new_v4().to_string();
            let content_version_id = Uuid::new_v4().to_string();
            let dataset_id = Uuid::new_v4().to_string();
            let dataset_version_id = Uuid::new_v4().to_string();
            connection.execute(
                "INSERT INTO documents (
                    document_id, storage_state, display_name, created_at_ms
                ) VALUES (?1, 'LINKED', 'fixture.csv', 1)",
                [&document_id],
            ).expect("document");
            connection.execute(
                "INSERT INTO file_instances (
                    file_instance_id, document_id, storage_root_id,
                    relative_path_native, path_native_encoding,
                    relative_path_display, relative_path_search,
                    size_bytes, first_seen_at_ms, last_seen_at_ms, availability_state
                 ) VALUES (?1, ?2, ?3, X'666978747572652E637376', 'test',
                           'fixture.csv', 'fixture.csv', 5, 1, 1, 'AVAILABLE')",
                params![&file_instance_id, &document_id, &root_id],
            ).expect("file instance");
            connection.execute(
                "INSERT INTO content_versions (
                    content_version_id, document_id, file_instance_id,
                    observed_at_ms, size_bytes, sha256,
                    verification_state, source_stable_during_read
                ) VALUES (?1, ?2, ?3, 1, 5, ?4, 'HASH_VERIFIED', 1)",
                params![&content_version_id, &document_id, &file_instance_id, vec![index + 1u8; 32]],
            ).expect("observed content version");
            connection.execute(
                "INSERT INTO normal_data_datasets (
                    normal_data_dataset_id, normal_data_workspace_id, name, created_at_ms
                ) VALUES (?1, ?2, 'Source', 1)",
                params![&dataset_id, &workspace.normal_data_workspace_id],
            ).expect("dataset");
            connection.execute(
                "INSERT INTO normal_data_dataset_versions (
                    normal_data_dataset_version_id, normal_data_dataset_id,
                    version_number, document_id, file_instance_id, content_version_id,
                    source_observed_at_ms, source_size_bytes,
                    source_verification_state, source_stable_during_read,
                    source_sha256, created_at_ms
                ) VALUES (?1, ?2, 1, ?3, ?4, ?5, 1, 5, 'HASH_VERIFIED', 1, ?6, 1)",
                params![&dataset_version_id, &dataset_id, &document_id,
                        &file_instance_id, &content_version_id, vec![index + 1u8; 32]],
            ).expect("versioned source");
            normal_data_datasets::declare_column(
                &database_path, &dataset_version_id, "Return Month", "FILING_PERIOD", "PERIOD",
            ).expect("declare filing period");
            normal_data_datasets::declare_column(
                &database_path, &dataset_version_id, "Invoice Date", "INVOICE_DATE", "DATE",
            ).expect("declare invoice date");
            normal_data_datasets::declare_column(
                &database_path, &dataset_version_id, "Taxable", "NUMERIC_VALUE", "DECIMAL",
            ).expect("declare numeric value");
            versions.push(dataset_version_id);
        }

        let recipe = create_recipe(
            &database_path,
            RecipeDefinition {
                normal_data_workspace_id: &workspace.normal_data_workspace_id,
                name: "Filing month comparison",
                dataset_a_version_id: &versions[0],
                dataset_b_version_id: &versions[1],
                period_basis: "FILING_PERIOD",
                amount_columns: &["Taxable".to_string()],
                tolerance_minor_units: 5,
            },
        ).expect("create bound recipe");
        assert_eq!(recipe.version_number, 1);
        assert_eq!(recipe.period_basis, "FILING_PERIOD");
        let listed = list_recipes(&database_path, &workspace.normal_data_workspace_id)
            .expect("list recipes");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].normal_data_comparison_recipe_version_id,
                   recipe.normal_data_comparison_recipe_version_id);
        assert!(list_recipes(&database_path, &other_workspace.normal_data_workspace_id)
            .expect("other workspace list").is_empty());

        assert!(create_recipe(
            &database_path,
            RecipeDefinition {
                normal_data_workspace_id: &other_workspace.normal_data_workspace_id,
                name: "Cross workspace",
                dataset_a_version_id: &versions[0],
                dataset_b_version_id: &versions[1],
                period_basis: "FILING_PERIOD",
                amount_columns: &["Taxable".to_string()],
                tolerance_minor_units: 0,
            },
        ).is_err());
        assert!(create_recipe(
            &database_path,
            RecipeDefinition {
                normal_data_workspace_id: &workspace.normal_data_workspace_id,
                name: "Incorrect numeric field",
                dataset_a_version_id: &versions[0],
                dataset_b_version_id: &versions[1],
                period_basis: "FILING_PERIOD",
                amount_columns: &["Missing".to_string()],
                tolerance_minor_units: 0,
            },
        ).is_err());
        assert!(connection.execute(
            "UPDATE normal_data_comparison_recipe_versions SET name = 'tampered'
             WHERE normal_data_comparison_recipe_version_id = ?1",
            [&recipe.normal_data_comparison_recipe_version_id],
        ).is_err());
        assert!(connection.execute(
            "DELETE FROM normal_data_comparison_recipes
             WHERE normal_data_comparison_recipe_id = ?1",
            [&recipe.normal_data_comparison_recipe_id],
        ).is_err());
        let events = persistence::count_audit_events_for_test(
            &database_path, "NORMAL_DATA_RECIPE_CREATED",
            &recipe.normal_data_comparison_recipe_id,
        ).expect("creation audit event");
        assert_eq!(events, 1);
        drop(connection);
        fs::remove_dir_all(folder).expect("remove test fixture");
    }

    #[test]
    fn recipe_configuration_requires_confirmed_non_duplicate_fields() {
        let id_a = Uuid::new_v4().to_string();
        let id_b = Uuid::new_v4().to_string();
        let columns = vec!["Taxable value".to_string()];
        let mut definition = RecipeDefinition {
            normal_data_workspace_id: "workspace",
            name: " Filing-vs-Invoice ",
            dataset_a_version_id: &id_a,
            dataset_b_version_id: &id_b,
            period_basis: "FILING_PERIOD",
            amount_columns: &columns,
            tolerance_minor_units: 0,
        };
        let (name, fields, role, _) = validate_definition(&definition).expect("valid definition");
        assert_eq!(name, "Filing-vs-Invoice");
        assert_eq!(fields, vec!["Taxable value"]);
        assert_eq!(role, "FILING_PERIOD");
        definition.period_basis = "INVOICE_MONTH";
        let (_, _, date_role, date_type) = validate_definition(&definition).expect("date basis");
        assert_eq!(date_role, "INVOICE_DATE");
        assert_eq!(date_type, "DATE");
        definition.period_basis = "BAD_PERIOD";
        assert!(validate_definition(&definition).is_err());
        definition.period_basis = "FILING_PERIOD";
        definition.tolerance_minor_units = -1;
        assert!(validate_definition(&definition).is_err());
        definition.tolerance_minor_units = 0;
        // Distinct names are checked case-insensitively by the definition validator.
        assert!(validate_definition(&RecipeDefinition {
            amount_columns: &["Tax".to_string(), "tax".to_string()],
            ..definition
        }).is_err());
    }
}
