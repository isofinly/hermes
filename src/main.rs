use std::num::{NonZeroU16, NonZeroU64};
use std::thread;

use clap::Parser;
use hermes::db::persist_scan_results;
use hermes::masscan_cli::{
    MasscanCommand, MasscanError, NonEmptyList, PortSelection, PortSpec, TargetSpec,
};
use hermes::notifications::{EmailConfig, send_results_email};
use hermes::results::{PortStatusRecord, parse_ndjson_with_threads, pretty_print_records};

#[derive(Parser, Debug)]
#[command(author, version, about = "Masscan runner with SQLite persistence")]
struct Cli {
    #[arg(long = "target", required = true, num_args = 1..)]
    targets: Vec<String>,

    #[arg(long, default_value = "41641,443,80,22")]
    ports: String,

    #[arg(long, default_value_t = 2000)]
    rate: u64,

    #[arg(long, default_value_t = 1)]
    max_retries: u32,

    #[arg(long, default_value_t = 0)]
    wait: u32,

    #[arg(long, default_value = "results.sqlite3")]
    database: String,

    #[arg(long, default_value_t = 1)]
    masscan_processes: usize,

    #[arg(long, default_value_t = 1)]
    parse_threads: usize,

    #[arg(long = "masscan-arg", allow_hyphen_values = true)]
    masscan_args: Vec<String>,

    #[arg(long)]
    smtp_server: Option<String>,

    #[arg(long, default_value_t = 587)]
    smtp_port: u16,

    #[arg(long)]
    smtp_username: Option<String>,

    #[arg(long)]
    smtp_password: Option<String>,

    #[arg(long)]
    email_from: Option<String>,

    #[arg(long)]
    email_to: Option<String>,

    #[arg(long, default_value = "Masscan Results")]
    email_subject: String,
}

fn main() {
    let cli = Cli::parse();

    let email_config = build_email_config(&cli);
    let targets = build_targets(&cli.targets);
    let ports = parse_port_selection(&cli.ports);
    let rate = NonZeroU64::new(cli.rate).ok_or("rate must be non-zero");

    if cli.masscan_processes == 0 {
        eprintln!("--masscan-processes must be greater than zero");
        std::process::exit(1);
    }

    if cli.parse_threads == 0 {
        eprintln!("--parse-threads must be greater than zero");
        std::process::exit(1);
    }

    let rows = targets
        .and_then(|targets| ports.map(|ports| (targets, ports)))
        .map_err(|err| err.to_string())
        .and_then(|(targets, ports)| {
            rate.map_err(str::to_string)
                .map(|rate| (targets, ports, rate))
        })
        .and_then(|(targets, ports, rate)| {
            run_masscan_workers(
                targets,
                ports,
                rate,
                cli.max_retries,
                cli.wait,
                cli.masscan_processes,
                cli.parse_threads,
                &cli.masscan_args,
            )
        });

    if let Err(err) = rows.and_then(|rows| {
        pretty_print_records(&rows);
        persist_scan_results(&cli.database, &rows)
            .map(|saved_count| {
                if saved_count > 0 {
                    println!("Saved {saved_count} scan rows into {}", cli.database);
                }
            })
            .map_err(|err| err.to_string())?;

        if let Some(config) = email_config.as_ref().map_err(|err| err.to_string())? {
            send_results_email(config, &rows)?;
            println!("Sent scan results via email to {}", config.to);
        }

        Ok(())
    }) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn build_targets(values: &[String]) -> Result<Vec<TargetSpec>, MasscanError> {
    values
        .iter()
        .map(|value| TargetSpec::new(value.clone()))
        .collect()
}

fn parse_port_selection(ports: &str) -> Result<PortSelection, MasscanError> {
    let mut parsed_ports = Vec::new();

    for fragment in ports.split(',') {
        let trimmed = fragment.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some((start, end)) = trimmed.split_once('-') {
            let start = start
                .parse::<u16>()
                .ok()
                .and_then(NonZeroU16::new)
                .ok_or(MasscanError::EmptyValue("port range start"))?;
            let end = end
                .parse::<u16>()
                .ok()
                .and_then(NonZeroU16::new)
                .ok_or(MasscanError::EmptyValue("port range end"))?;
            parsed_ports.push(PortSpec::range(start, end)?);
            continue;
        }

        let port = trimmed
            .parse::<u16>()
            .ok()
            .and_then(NonZeroU16::new)
            .ok_or(MasscanError::EmptyValue("port"))?;
        parsed_ports.push(PortSpec::single(port));
    }

    let mut iter = parsed_ports.into_iter();
    let first = iter.next().ok_or(MasscanError::EmptyValue("port"))?;
    let mut selection = PortSelection::new(first);
    for port in iter {
        selection = selection.push(port);
    }

    Ok(selection)
}

fn build_email_config(cli: &Cli) -> Result<Option<EmailConfig>, String> {
    let any_email_arg = cli.smtp_server.is_some()
        || cli.smtp_username.is_some()
        || cli.smtp_password.is_some()
        || cli.email_from.is_some()
        || cli.email_to.is_some();

    if !any_email_arg {
        return Ok(None);
    }

    let smtp_server = cli
        .smtp_server
        .clone()
        .ok_or("missing --smtp-server for email notifications")?;
    let smtp_username = cli
        .smtp_username
        .clone()
        .ok_or("missing --smtp-username for email notifications")?;
    let smtp_password = cli
        .smtp_password
        .clone()
        .ok_or("missing --smtp-password for email notifications")?;
    let from = cli
        .email_from
        .clone()
        .ok_or("missing --email-from for email notifications")?;
    let to = cli
        .email_to
        .clone()
        .ok_or("missing --email-to for email notifications")?;

    Ok(Some(EmailConfig {
        smtp_server,
        smtp_port: cli.smtp_port,
        smtp_username,
        smtp_password,
        from,
        to,
        subject: cli.email_subject.clone(),
    }))
}

#[allow(clippy::too_many_arguments)]
fn run_masscan_workers(
    targets: Vec<TargetSpec>,
    ports: PortSelection,
    rate: NonZeroU64,
    max_retries: u32,
    wait: u32,
    masscan_processes: usize,
    parse_threads: usize,
    extra_args: &[String],
) -> Result<Vec<PortStatusRecord>, String> {
    let target_batches = split_targets_for_workers(targets, masscan_processes)?;

    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(target_batches.len());

        for target_batch in target_batches {
            let ports = ports.clone();
            let extra_args = extra_args.to_vec();
            handles.push(scope.spawn(move || {
                run_single_masscan_worker(
                    target_batch,
                    ports,
                    rate,
                    max_retries,
                    wait,
                    parse_threads,
                    &extra_args,
                )
            }));
        }

        let mut rows = Vec::new();
        for (worker_index, handle) in handles.into_iter().enumerate() {
            let worker_rows = handle
                .join()
                .map_err(|_| format!("masscan worker {worker_index} panicked"))??;
            rows.extend(worker_rows);
        }

        Ok(rows)
    })
}

fn run_single_masscan_worker(
    targets: NonEmptyList<TargetSpec>,
    ports: PortSelection,
    rate: NonZeroU64,
    max_retries: u32,
    wait: u32,
    parse_threads: usize,
    extra_args: &[String],
) -> Result<Vec<PortStatusRecord>, String> {
    let mut command = MasscanCommand::scan(targets, ports)
        .rate(rate)
        .max_retries(max_retries)
        .wait(wait)
        .output_ndjson("-")
        .map_err(|err| err.to_string())?;

    for extra_arg in extra_args {
        command = command.arg(extra_arg);
    }

    let ndjson_output = command
        .invoke_subprocess_capture_stdout()
        .map_err(|err| err.to_string())?;
    parse_ndjson_with_threads(&ndjson_output, parse_threads)
}

fn split_targets_for_workers(
    targets: Vec<TargetSpec>,
    masscan_processes: usize,
) -> Result<Vec<NonEmptyList<TargetSpec>>, String> {
    if targets.is_empty() {
        return Err("at least one --target is required".to_string());
    }

    let worker_count = masscan_processes.min(targets.len());
    let mut buckets: Vec<Vec<TargetSpec>> = (0..worker_count).map(|_| Vec::new()).collect();

    for (index, target) in targets.into_iter().enumerate() {
        buckets[index % worker_count].push(target);
    }

    buckets
        .into_iter()
        .map(non_empty_targets)
        .collect::<Result<Vec<_>, _>>()
}

fn non_empty_targets(targets: Vec<TargetSpec>) -> Result<NonEmptyList<TargetSpec>, String> {
    let mut iter = targets.into_iter();
    let Some(first) = iter.next() else {
        return Err("cannot start worker with empty target list".to_string());
    };

    let mut list = NonEmptyList::new(first);
    for target in iter {
        list = list.push(target);
    }

    Ok(list)
}
