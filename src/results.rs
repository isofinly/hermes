use std::fs;

use serde_json::Value;

#[derive(Clone, Debug)]
pub struct PortStatusRecord {
    pub ip: String,
    pub proto: String,
    pub port: i32,
    pub status: String,
    pub reason: String,
}

pub fn parse_ndjson(output_path: &str) -> Result<Vec<PortStatusRecord>, String> {
    let content = fs::read_to_string(output_path)
        .map_err(|err| format!("failed reading {output_path}: {err}"))?;

    let mut rows = Vec::new();

    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed: Value = serde_json::from_str(trimmed)
            .map_err(|err| format!("invalid ndjson at line {}: {err}", line_index + 1))?;

        let ip = parsed
            .get("ip")
            .and_then(Value::as_str)
            .unwrap_or("<unknown-ip>")
            .to_string();

        if let Some(ports) = parsed.get("port").and_then(Value::as_array) {
            for port in ports {
                let proto = port
                    .get("proto")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string();
                let port_number = port
                    .get("port")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .try_into()
                    .unwrap_or(0);
                let status = port
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let reason = port
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("n/a")
                    .to_string();

                rows.push(PortStatusRecord {
                    ip: ip.clone(),
                    proto,
                    port: port_number,
                    status,
                    reason,
                });
            }
            continue;
        }

        if parsed.get("rec_type").and_then(Value::as_str) == Some("status") {
            let proto = parsed
                .get("proto")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            let port_number = parsed
                .get("port")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .try_into()
                .unwrap_or(0);
            let status = parsed
                .get("data")
                .and_then(|data| data.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let reason = parsed
                .get("data")
                .and_then(|data| data.get("reason"))
                .and_then(Value::as_str)
                .unwrap_or("n/a")
                .to_string();

            rows.push(PortStatusRecord {
                ip,
                proto,
                port: port_number,
                status,
                reason,
            });
        }
    }

    Ok(rows)
}

pub fn pretty_print_records(records: &[PortStatusRecord]) {
    if records.is_empty() {
        println!("No open ports found for requested targets");
        return;
    }

    for row in records {
        let port_label = if row.port == 0 {
            "?".to_string()
        } else {
            row.port.to_string()
        };
        println!(
            "{:>15}  {:<4}  {:>5}  status={:<7} reason={}",
            row.ip, row.proto, port_label, row.status, row.reason
        );
    }
}

pub fn pretty_print_ndjson(output_path: &str) -> Result<(), String> {
    let rows = parse_ndjson(output_path)?;
    pretty_print_records(&rows);
    Ok(())
}
