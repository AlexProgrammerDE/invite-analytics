use anyhow::{Context as _, bail};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::invite_tracking::normalize_invite_code;
use crate::models::ExportInvite;

const MAX_IMPORT_ROWS: usize = 1_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedInvite {
    pub invite_code: String,
    pub primary_source: String,
    pub secondary_source: String,
    pub creator_id: String,
    pub created_at: Option<NaiveDateTime>,
    pub role_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CsvRecord {
    #[serde(rename = "Invite Code")]
    invite_code: String,
    #[serde(rename = "Primary Source")]
    primary_source: String,
    #[serde(rename = "Secondary Source")]
    secondary_source: String,
    #[serde(rename = "Link Creator ID")]
    creator_id: String,
    #[serde(rename = "Creation Time")]
    creation_time: String,
    #[serde(rename = "Roles to Give on join")]
    role_ids: String,
}

pub fn parse_import(bytes: &[u8]) -> anyhow::Result<Vec<ImportedInvite>> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(bytes);
    let mut rows = Vec::new();

    for (index, result) in reader.deserialize::<CsvRecord>().enumerate() {
        if index >= MAX_IMPORT_ROWS {
            bail!("CSV files can contain at most {MAX_IMPORT_ROWS} rows");
        }

        let record = result.with_context(|| format!("invalid CSV record on row {}", index + 2))?;
        let invite_code = normalize_invite_code(&record.invite_code);
        if invite_code.is_empty()
            || record.primary_source.is_empty()
            || record.secondary_source.is_empty()
            || record.creator_id.is_empty()
        {
            bail!("row {} has a missing required value", index + 2);
        }

        rows.push(ImportedInvite {
            invite_code,
            primary_source: record.primary_source,
            secondary_source: record.secondary_source,
            creator_id: record.creator_id,
            created_at: parse_date(&record.creation_time)
                .with_context(|| format!("invalid creation time on row {}", index + 2))?,
            role_ids: record
                .role_ids
                .split(';')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        });
    }

    Ok(rows)
}

pub fn write_export(invites: &[ExportInvite]) -> anyhow::Result<Vec<u8>> {
    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    for invite in invites {
        writer.serialize(CsvRecord {
            invite_code: invite.invite_code.clone(),
            primary_source: invite.primary_source.clone(),
            secondary_source: invite.secondary_source.clone(),
            creator_id: invite.created_by.clone(),
            creation_time: invite.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            role_ids: invite.role_ids.join(";"),
        })?;
    }
    writer
        .into_inner()
        .context("failed to finish the CSV export")
}

fn parse_date(value: &str) -> anyhow::Result<Option<NaiveDateTime>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    if let Ok(value) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
        return Ok(Some(value));
    }
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Ok(Some(value.with_timezone(&Utc).naive_utc()));
    }

    bail!("expected YYYY-MM-DD HH:MM:SS or an RFC 3339 timestamp")
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{parse_import, write_export};
    use crate::models::ExportInvite;

    #[test]
    fn export_round_trips_quoted_values() {
        let created_at = NaiveDate::from_ymd_opt(2026, 7, 23)
            .unwrap()
            .and_hms_opt(14, 30, 0)
            .unwrap();
        let original = ExportInvite {
            invite_code: "summer".to_owned(),
            primary_source: "Video, social".to_owned(),
            secondary_source: "\"Launch\" description".to_owned(),
            created_by: "123".to_owned(),
            created_at,
            role_ids: vec!["456".to_owned(), "789".to_owned()],
        };

        let bytes = write_export(std::slice::from_ref(&original)).unwrap();
        let parsed = parse_import(&bytes).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].invite_code, original.invite_code);
        assert_eq!(parsed[0].primary_source, original.primary_source);
        assert_eq!(parsed[0].secondary_source, original.secondary_source);
        assert_eq!(parsed[0].creator_id, original.created_by);
        assert_eq!(parsed[0].created_at, Some(original.created_at));
        assert_eq!(parsed[0].role_ids, original.role_ids);
    }
}
