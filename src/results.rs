use rayon::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct PortStatusRecord {
    pub ip: String,
    pub proto: String,
    pub port: i32,
    pub status: String,
    pub reason: String,
}

pub fn parse_ndjson_with_threads(
    ndjson_content: &str,
    thread_count: usize,
) -> Result<Vec<PortStatusRecord>, String> {
    let indexed_lines: Vec<(usize, String)> = ndjson_content
        .lines()
        .enumerate()
        .map(|(index, line)| (index, line.to_string()))
        .collect();

    let worker_count = thread_count.max(1);
    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(worker_count)
        .build()
        .map_err(|err| format!("failed to build rayon thread pool: {err}"))?;

    let chunks = thread_pool.install(|| {
        indexed_lines
            .par_iter()
            .map(|(line_index, line)| parse_line(line_index + 1, line))
            .collect::<Result<Vec<_>, _>>()
    })?;

    Ok(chunks.into_iter().flatten().collect())
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

fn parse_line(line_number: usize, line: &str) -> Result<Vec<PortStatusRecord>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let parsed: Value = serde_json::from_str(trimmed)
        .map_err(|err| format!("invalid ndjson at line {line_number}: {err}"))?;

    let ip = parsed
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("<unknown-ip>")
        .to_string();

    if let Some(ports) = parsed.get("port").and_then(Value::as_array) {
        let records = ports
            .iter()
            .map(|port| PortStatusRecord {
                ip: ip.clone(),
                proto: port
                    .get("proto")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string(),
                port: port
                    .get("port")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .try_into()
                    .unwrap_or(0),
                status: port
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                reason: port
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("n/a")
                    .to_string(),
            })
            .collect();
        return Ok(records);
    }

    if parsed.get("rec_type").and_then(Value::as_str) == Some("status") {
        return Ok(vec![PortStatusRecord {
            ip,
            proto: parsed
                .get("proto")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string(),
            port: parsed
                .get("port")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .try_into()
                .unwrap_or(0),
            status: parsed
                .get("data")
                .and_then(|data| data.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            reason: parsed
                .get("data")
                .and_then(|data| data.get("reason"))
                .and_then(Value::as_str)
                .unwrap_or("n/a")
                .to_string(),
        }]);
    }

    Ok(Vec::new())
}
