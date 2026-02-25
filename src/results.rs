use std::fs;

use serde_json::Value;

pub fn pretty_print_ndjson(output_path: &str) -> Result<(), String> {
    let content = fs::read_to_string(output_path)
        .map_err(|err| format!("failed reading {output_path}: {err}"))?;

    let mut has_rows = false;

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
            .unwrap_or("<unknown-ip>");

        if let Some(ports) = parsed.get("port").and_then(Value::as_array) {
            for port in ports {
                let proto = port.get("proto").and_then(Value::as_str).unwrap_or("?");
                let port_number = port
                    .get("port")
                    .and_then(Value::as_u64)
                    .map_or_else(|| "?".to_string(), |v| v.to_string());
                let status = port
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let reason = port.get("reason").and_then(Value::as_str).unwrap_or("n/a");

                println!(
                    "{ip:>15}  {proto:<4}  {port_number:>5}  status={status:<7} reason={reason}"
                );
                has_rows = true;
            }
            continue;
        }

        if parsed.get("rec_type").and_then(Value::as_str) == Some("status") {
            let proto = parsed.get("proto").and_then(Value::as_str).unwrap_or("?");
            let port_number = parsed
                .get("port")
                .and_then(Value::as_u64)
                .map_or_else(|| "?".to_string(), |v| v.to_string());

            let status = parsed
                .get("data")
                .and_then(|data| data.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let reason = parsed
                .get("data")
                .and_then(|data| data.get("reason"))
                .and_then(Value::as_str)
                .unwrap_or("n/a");

            println!("{ip:>15}  {proto:<4}  {port_number:>5}  status={status:<7} reason={reason}");
            has_rows = true;
        }
    }

    if !has_rows {
        println!("No open ports found for requested targets");
    }

    Ok(())
}
