use std::num::{NonZeroU16, NonZeroU64};

use hermes::db::persist_scan_results;
use hermes::masscan_cli::{
    MasscanCommand, MasscanError, NonEmptyList, PortSelection, PortSpec, TargetSpec,
};
use hermes::results::{parse_ndjson, pretty_print_records};

fn main() {
    let output_path = "results.ndjson";
    let database_path = "results.sqlite3";

    let targets = TargetSpec::new("127.0.0.1/24").map(NonEmptyList::new);
    let ports = build_example_ports();
    let rate = NonZeroU64::new(2000).ok_or("rate must be non-zero");

    let command = targets
        .and_then(|targets| ports.map(|ports| (targets, ports)))
        .map_err(|err| err.to_string())
        .and_then(|(targets, ports)| {
            rate.map_err(str::to_string).and_then(|rate| {
                MasscanCommand::scan(targets, ports)
                    .rate(rate)
                    .max_retries(1)
                    .wait(0)
                    .output_ndjson(output_path)
                    .map_err(|err| err.to_string())
            })
        });

    if let Err(err) = command
        .and_then(|command| command.invoke().map_err(|err| err.to_string()))
        .and_then(|_| parse_ndjson(output_path))
        .and_then(|rows| {
            pretty_print_records(&rows);
            persist_scan_results(database_path, &rows)
                .map(|saved_count| {
                    if saved_count > 0 {
                        println!("Saved {saved_count} scan rows into {database_path}");
                    }
                })
                .map_err(|err| err.to_string())
        })
    {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn build_example_ports() -> Result<PortSelection, MasscanError> {
    let p41641 = NonZeroU16::new(41641).ok_or(MasscanError::EmptyValue("port"))?;
    let p443 = NonZeroU16::new(443).ok_or(MasscanError::EmptyValue("port"))?;
    let p80 = NonZeroU16::new(80).ok_or(MasscanError::EmptyValue("port"))?;
    let p22 = NonZeroU16::new(22).ok_or(MasscanError::EmptyValue("port"))?;

    Ok(PortSelection::new(PortSpec::single(p41641))
        .push(PortSpec::single(p443))
        .push(PortSpec::single(p80))
        .push(PortSpec::single(p22)))
}
