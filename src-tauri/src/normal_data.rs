//! Engagement-independent normal-data workspace metadata.
//! No filesystem path, controlled-evidence capture, or deterministic run is accepted here.
use crate::persistence::{self, PersistenceError};
use rusqlite::{params, TransactionBehavior};
use serde::Serialize;
use serde_json::json;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    pub normal_data_workspace_id: String,
    pub normal_data_workspace_version_id: String,
    pub version_number: i64,
    pub name: String,
    pub description: Option<String>,
    pub client_id: Option<String>,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub struct WorkspaceDefinition<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub client_id: Option<&'a str>,
    pub period_start: Option<&'a str>,
    pub period_end: Option<&'a str>,
}

fn invalid(message: impl Into<String>) -> PersistenceError {
    PersistenceError::Configuration(message.into())
}

fn optional_description(value: Option<&str>) -> Result<Option<String>, PersistenceError> {
    let Some(value) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    if value.chars().count() > 8192
        || value
            .chars()
            .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
    {
        return Err(invalid(
            "normal data workspace description is invalid or too long",
        ));
    }
    Ok(Some(value.to_string()))
}

fn calendar_date(value: Option<&str>, label: &str) -> Result<Option<String>, PersistenceError> {
    let Some(value) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..].iter().all(u8::is_ascii_digit)
    {
        return Err(invalid(format!("{label} must use YYYY-MM-DD format")));
    }
    let year: u32 = value[..4].parse().map_err(|_| invalid("invalid year"))?;
    let month: u32 = value[5..7].parse().map_err(|_| invalid("invalid month"))?;
    let day: u32 = value[8..].parse().map_err(|_| invalid("invalid day"))?;
    if year == 0 || !(1..=12).contains(&month) {
        return Err(invalid(format!("{label} is not a valid calendar date")));
    }
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    let maximum = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day == 0 || day > maximum {
        return Err(invalid(format!("{label} is not a valid calendar date")));
    }
    Ok(Some(value.to_string()))
}

pub fn create_workspace(
    database_path: &Path,
    input: WorkspaceDefinition<'_>,
) -> Result<WorkspaceRecord, PersistenceError> {
    let name = input.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > 240 || name.chars().any(char::is_control) {
        return Err(invalid(
            "normal data workspace name must have 1 to 240 printable characters",
        ));
    }
    let description = optional_description(input.description)?;
    let period_start = calendar_date(input.period_start, "period start")?;
    let period_end = calendar_date(input.period_end, "period end")?;
    if period_start.is_some() != period_end.is_some() {
        return Err(invalid(
            "period start and end must both be supplied or both omitted",
        ));
    }
    if period_start > period_end {
        return Err(invalid("period start cannot be after period end"));
    }
    let client_id = input
        .client_id
        .map(|value| {
            Uuid::parse_str(value)
                .map(|uuid| uuid.to_string())
                .map_err(|_| invalid("client ID must be a UUID"))
        })
        .transpose()?;

    let mut connection = persistence::open_configured_connection(database_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if let Some(client_id) = client_id.as_deref() {
        let active: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM clients WHERE client_id = ?1 AND archived_at_ms IS NULL)",
            [client_id],
            |row| row.get(0),
        )?;
        if !active {
            return Err(invalid("optional client does not exist or is archived"));
        }
    }

    let normal_data_workspace_id = Uuid::new_v4().to_string();
    let normal_data_workspace_version_id = Uuid::new_v4().to_string();
    let now = persistence::now_unix_ms()?;
    transaction.execute(
        "INSERT INTO normal_data_workspaces (normal_data_workspace_id, created_at_ms)
         VALUES (?1, ?2)",
        params![&normal_data_workspace_id, now],
    )?;
    transaction.execute(
        "INSERT INTO normal_data_workspace_versions (
            normal_data_workspace_version_id, normal_data_workspace_id, version_number,
            name, description, client_id, period_start, period_end, created_at_ms
         ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            &normal_data_workspace_version_id,
            &normal_data_workspace_id,
            &name,
            &description,
            &client_id,
            &period_start,
            &period_end,
            now
        ],
    )?;
    transaction.execute(
        "INSERT INTO audit_events (
            audit_event_id, event_type, entity_type, entity_id,
            related_entity_type, related_entity_id, occurred_at_ms, actor_id, details_json
         ) VALUES (?1, 'NORMAL_DATA_WORKSPACE_CREATED', 'NORMAL_DATA_WORKSPACE',
                   ?2, ?3, ?4, ?5, NULL, ?6)",
        params![
            Uuid::new_v4().to_string(),
            &normal_data_workspace_id,
            client_id.as_ref().map(|_| "CLIENT"),
            &client_id,
            now,
            json!({"workspace_version_id": normal_data_workspace_version_id, "version_number": 1})
                .to_string()
        ],
    )?;
    transaction.commit()?;
    Ok(WorkspaceRecord {
        normal_data_workspace_id,
        normal_data_workspace_version_id,
        version_number: 1,
        name,
        description,
        client_id,
        period_start,
        period_end,
        created_at_ms: now,
        updated_at_ms: now,
    })
}

pub fn list_workspaces(database_path: &Path) -> Result<Vec<WorkspaceRecord>, PersistenceError> {
    let connection = persistence::open_configured_connection(database_path)?;
    let mut statement = connection.prepare(
        "SELECT w.normal_data_workspace_id, v.normal_data_workspace_version_id,
                v.version_number, v.name, v.description, v.client_id,
                v.period_start, v.period_end, w.created_at_ms, v.created_at_ms
         FROM normal_data_workspaces w
         JOIN normal_data_workspace_versions v
           ON v.normal_data_workspace_id = w.normal_data_workspace_id
          AND v.version_number = (
                SELECT MAX(v2.version_number)
                FROM normal_data_workspace_versions v2
                WHERE v2.normal_data_workspace_id = w.normal_data_workspace_id
          )
         ORDER BY w.created_at_ms DESC, w.normal_data_workspace_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(WorkspaceRecord {
            normal_data_workspace_id: row.get(0)?,
            normal_data_workspace_version_id: row.get(1)?,
            version_number: row.get(2)?,
            name: row.get(3)?,
            description: row.get(4)?,
            client_id: row.get(5)?,
            period_start: row.get(6)?,
            period_end: row.get(7)?,
            created_at_ms: row.get(8)?,
            updated_at_ms: row.get(9)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence;
    use std::fs;
    use std::path::PathBuf;

    struct Fixture {
        folder: PathBuf,
        path: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let folder = std::env::temp_dir().join(format!("pdox-normal-data-{}", Uuid::new_v4()));
            fs::create_dir_all(&folder).expect("create fixture");
            let path = folder.join("metadata.sqlite");
            persistence::initialize_database(&path).expect("create schema");
            Self { folder, path }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.folder);
        }
    }
    fn new_definition<'a>(name: &'a str) -> WorkspaceDefinition<'a> {
        WorkspaceDefinition {
            name,
            description: None,
            client_id: None,
            period_start: None,
            period_end: None,
        }
    }

    #[test]
    fn independent_workspace_round_trips_and_is_audited() {
        let fixture = Fixture::new();
        let workspace = create_workspace(&fixture.path, new_definition("  Ordinary   data  "))
            .expect("create without client, engagement or workpaper");
        assert_eq!(workspace.name, "Ordinary data");
        assert_eq!(workspace.version_number, 1);
        assert!(workspace.client_id.is_none());
        let again = list_workspaces(&fixture.path).expect("list workspaces");
        assert_eq!(again.len(), 1);
        assert_eq!(
            again[0].normal_data_workspace_id,
            workspace.normal_data_workspace_id
        );
        assert_eq!(
            again[0].normal_data_workspace_version_id,
            workspace.normal_data_workspace_version_id
        );
        let count = persistence::count_audit_events_for_test(
            &fixture.path,
            "NORMAL_DATA_WORKSPACE_CREATED",
            &workspace.normal_data_workspace_id,
        )
        .expect("audit event count");
        assert_eq!(count, 1);
    }

    #[test]
    fn optional_client_and_calendar_period_are_validated() {
        let fixture = Fixture::new();
        let client = persistence::create_client(&fixture.path, "Normal client").expect("client");
        let mut input = new_definition("August filing month");
        input.client_id = Some(&client.client_id);
        input.period_start = Some("2026-08-01");
        input.period_end = Some("2026-08-31");
        let created = create_workspace(&fixture.path, input).expect("valid workspace");
        assert_eq!(
            created.client_id.as_deref(),
            Some(client.client_id.as_str())
        );
        assert_eq!(created.period_start.as_deref(), Some("2026-08-01"));

        let mut invalid = new_definition("Invalid period");
        invalid.period_start = Some("2026-02-29");
        invalid.period_end = Some("2026-03-01");
        assert!(create_workspace(&fixture.path, invalid).is_err());

        let mut missing = new_definition("Missing client");
        let missing_id = Uuid::new_v4().to_string();
        missing.client_id = Some(&missing_id);
        assert!(create_workspace(&fixture.path, missing).is_err());

        let mut reverse = new_definition("Reverse period");
        reverse.period_start = Some("2026-09-01");
        reverse.period_end = Some("2026-08-01");
        assert!(create_workspace(&fixture.path, reverse).is_err());

        assert!(create_workspace(&fixture.path, new_definition("   ")).is_err());
        assert_eq!(list_workspaces(&fixture.path).expect("list").len(), 1);
    }

    #[test]
    fn persisted_identity_and_version_cannot_be_rewritten_or_deleted() {
        let fixture = Fixture::new();
        let workspace =
            create_workspace(&fixture.path, new_definition("Immutable")).expect("create workspace");
        let connection =
            persistence::open_configured_connection(&fixture.path).expect("open connection");
        assert!(connection.execute(
            "UPDATE normal_data_workspace_versions SET name = 'Tampered' WHERE normal_data_workspace_version_id = ?1",
            [&workspace.normal_data_workspace_version_id]
        ).is_err());
        assert!(connection.execute(
            "DELETE FROM normal_data_workspace_versions WHERE normal_data_workspace_version_id = ?1",
            [&workspace.normal_data_workspace_version_id]
        ).is_err());
        assert!(connection
            .execute(
                "DELETE FROM normal_data_workspaces WHERE normal_data_workspace_id = ?1",
                [&workspace.normal_data_workspace_id]
            )
            .is_err());
    }
}
