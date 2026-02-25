use std::num::{NonZeroU16, NonZeroU64};

use clap::Parser;
use hermes::db::persist_scan_results;
use hermes::masscan_cli::{
    MasscanCommand, MasscanError, NonEmptyList, PortSelection, PortSpec, TargetSpec,
};
use hermes::notifications::{EmailConfig, send_results_email};
use hermes::results::{parse_ndjson_with_threads, pretty_print_records};

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

    #[arg(long, default_value = "results.ndjson")]
    output: String,

    #[arg(long, default_value = "results.sqlite3")]
    database: String,

    #[arg(long, default_value_t = 1)]
    parse_threads: usize,

    #[arg(long = "masscan-arg")]
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

    if cli.parse_threads == 0 {
        eprintln!("--parse-threads must be greater than zero");
        std::process::exit(1);
    }

    let command = targets
        .and_then(|targets| ports.map(|ports| (targets, ports)))
        .map_err(|err| err.to_string())
        .and_then(|(targets, ports)| {
            rate.map_err(str::to_string).and_then(|rate| {
                let mut command = MasscanCommand::scan(targets, ports)
                    .rate(rate)
                    .max_retries(cli.max_retries)
                    .wait(cli.wait)
                    .output_ndjson(&cli.output)
                    .map_err(|err| err.to_string())?;

                for extra_arg in &cli.masscan_args {
                    command = command.arg(extra_arg.clone());
                }

                Ok(command)
            })
        });

    if let Err(err) = command
        .and_then(|command| command.invoke().map_err(|err| err.to_string()))
        .and_then(|_| parse_ndjson_with_threads(&cli.output, cli.parse_threads))
        .and_then(|rows| {
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
        })
    {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn build_targets(values: &[String]) -> Result<NonEmptyList<TargetSpec>, MasscanError> {
    let mut iter = values.iter();
    let first = iter
        .next()
        .ok_or(MasscanError::EmptyValue("target"))
        .and_then(|value| TargetSpec::new(value.clone()))?;

    let mut targets = NonEmptyList::new(first);
    for value in iter {
        targets = targets.push(TargetSpec::new(value.clone())?);
    }

    Ok(targets)
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
