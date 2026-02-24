use std::ffi::CString;
use std::fs;
use std::os::raw::c_char;

use hermes::masscan_api::raw::masscan_cli_main;
use serde_json::Value;

fn run_masscan_via_c_api(output_path: &str) -> Result<(), String> {
    let args = [
        "masscan",
        "127.0.0.1/24",
        "-p41641,443,80,22",
        "--rate",
        "2000",
        "--max-retries",
        "1",
        "--wait",
        "0",
        "-oD",
        output_path,
    ];

    let cstrings: Vec<CString> = args
        .iter()
        .map(|arg| CString::new(*arg).map_err(|_| format!("argument contains NUL: {arg}")))
        .collect::<Result<_, _>>()?;

    let mut argv: Vec<*mut c_char> = cstrings
        .iter()
        .map(|arg| arg.as_ptr() as *mut c_char)
        .collect();
    argv.push(std::ptr::null_mut());

    let exit_code = unsafe { masscan_cli_main(cstrings.len() as i32, argv.as_mut_ptr()) };

    if exit_code != 0 {
        return Err(format!(
            "masscan API returned non-zero exit code: {exit_code}. If this is a permission error, retry under sudo"
        ));
    }

    Ok(())
}

fn pretty_print_ndjson(output_path: &str) -> Result<(), String> {
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

        // Newer masscan NDJSON emits one record per event, e.g.:
        // {"ip":"127.0.0.1","port":443,"proto":"tcp","rec_type":"status","data":{"status":"open","reason":"syn-ack"}}
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

fn main() {
    let output_path = "results.ndjson";

    if let Err(err) =
        run_masscan_via_c_api(output_path).and_then(|_| pretty_print_ndjson(output_path))
    {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
