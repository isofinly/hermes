use std::time::{SystemTime, UNIX_EPOCH};

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::results::PortStatusRecord;

diesel::table! {
    scan_results (id) {
        id -> Integer,
        scanned_at_epoch_s -> BigInt,
        ip -> Text,
        proto -> Text,
        port -> Integer,
        status -> Text,
        reason -> Text,
    }
}

#[derive(Insertable)]
#[diesel(table_name = scan_results)]
struct NewScanResult<'a> {
    scanned_at_epoch_s: i64,
    ip: &'a str,
    proto: &'a str,
    port: i32,
    status: &'a str,
    reason: &'a str,
}

pub fn persist_scan_results(db_path: &str, records: &[PortStatusRecord]) -> Result<usize, String> {
    if records.is_empty() {
        return Ok(0);
    }

    let mut connection = SqliteConnection::establish(db_path)
        .map_err(|err| format!("failed to open sqlite database {db_path}: {err}"))?;

    diesel::sql_query(
        "CREATE TABLE IF NOT EXISTS scan_results (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            scanned_at_epoch_s INTEGER NOT NULL,\
            ip TEXT NOT NULL,\
            proto TEXT NOT NULL,\
            port INTEGER NOT NULL,\
            status TEXT NOT NULL,\
            reason TEXT NOT NULL\
        )",
    )
    .execute(&mut connection)
    .map_err(|err| format!("failed to ensure schema in {db_path}: {err}"))?;

    let now_epoch_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_secs() as i64;

    let new_rows: Vec<NewScanResult<'_>> = records
        .iter()
        .map(|record| NewScanResult {
            scanned_at_epoch_s: now_epoch_s,
            ip: &record.ip,
            proto: &record.proto,
            port: record.port,
            status: &record.status,
            reason: &record.reason,
        })
        .collect();

    diesel::insert_into(scan_results::table)
        .values(&new_rows)
        .execute(&mut connection)
        .map_err(|err| format!("failed to insert scan results into {db_path}: {err}"))
}
