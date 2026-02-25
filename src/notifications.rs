use std::str::FromStr;

use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

use crate::results::PortStatusRecord;

#[derive(Clone, Debug)]
pub struct EmailConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from: String,
    pub to: String,
    pub subject: String,
}

pub fn send_results_email(config: &EmailConfig, rows: &[PortStatusRecord]) -> Result<(), String> {
    let from = Mailbox::from_str(&config.from)
        .map_err(|err| format!("invalid sender email '{}': {err}", config.from))?;
    let to = Mailbox::from_str(&config.to)
        .map_err(|err| format!("invalid recipient email '{}': {err}", config.to))?;

    let body = build_email_body(rows);

    let email = Message::builder()
        .from(from)
        .to(to)
        .subject(&config.subject)
        .body(body)
        .map_err(|err| format!("failed to build email: {err}"))?;

    let credentials = Credentials::new(config.smtp_username.clone(), config.smtp_password.clone());

    let transport = SmtpTransport::relay(&config.smtp_server)
        .map_err(|err| {
            format!(
                "failed to configure SMTP relay {}: {err}",
                config.smtp_server
            )
        })?
        .port(config.smtp_port)
        .credentials(credentials)
        .build();

    transport
        .send(&email)
        .map_err(|err| format!("failed to send email: {err}"))?;

    Ok(())
}

fn build_email_body(rows: &[PortStatusRecord]) -> String {
    if rows.is_empty() {
        return "No open ports found for requested targets.".to_string();
    }

    let mut body = String::from("Open ports detected:\n\n");
    for row in rows {
        let line = format!(
            "{} {} {} status={} reason={}\n",
            row.ip, row.proto, row.port, row.status, row.reason
        );
        body.push_str(&line);
    }

    // TODO: add richer HTML rendering and aggregation once notification format stabilizes.
    body
}
