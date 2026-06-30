//! Self-hosted Slack adapter and Socket Mode transport for the local Rivora CLI.
//!
//! Networking and token handling stay here so `rivora-slack` remains a pure,
//! deterministic rendering and interaction-contract crate.

use std::collections::HashSet;
use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::thread;
use std::time::Duration;

use rivora_errors::{Result, RivoraError};
use rivora_memory::{MemoryRecord, MemoryStatus};
use rivora_slack::{
    SlackFeedbackAction, SlackFeedbackActionKind, SlackMentionRequest, SlackReliabilityMemoryApp,
};
use serde::{Deserialize, Serialize};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};

use crate::{ask, display_path, LocalMemoryStore, StoreSnapshot, DEFAULT_TIMESTAMP};

const SLACK_CONNECTIONS_OPEN_URL: &str = "https://slack.com/api/apps.connections.open";
const SLACK_POST_MESSAGE_URL: &str = "https://slack.com/api/chat.postMessage";
const SLACK_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const SLACK_MAX_RECONNECT_ATTEMPTS: u32 = 5;
const SLACK_RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(30);
const SLACK_MESSAGE_LIMIT: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlackCommand {
    Dev(SlackDevOptions),
    Doctor(SlackDoctorOptions),
    Socket(SlackSocketOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackDevOptions {
    pub text: String,
    pub channel: String,
    pub user: String,
    pub bot_user_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlackSocketOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlackDoctorOptions {
    pub live: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SlackTokenConfig {
    bot_token: String,
    app_token: String,
    signing_secret: String,
}

impl std::fmt::Debug for SlackTokenConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackTokenConfig")
            .field("bot_token", &"[redacted]")
            .field("app_token", &"[redacted]")
            .field("signing", &"[redacted]")
            .finish()
    }
}

impl SlackTokenConfig {
    pub fn from_env() -> Result<Self> {
        Self::from_values(
            std::env::var("SLACK_BOT_TOKEN").ok(),
            std::env::var("SLACK_APP_TOKEN").ok(),
            std::env::var("SLACK_SIGNING_SECRET").ok(),
        )
    }

    pub fn from_values(
        bot_token: Option<String>,
        app_token: Option<String>,
        signing_secret: Option<String>,
    ) -> Result<Self> {
        let missing = [
            ("SLACK_BOT_TOKEN", bot_token.as_deref()),
            ("SLACK_APP_TOKEN", app_token.as_deref()),
            ("SLACK_SIGNING_SECRET", signing_secret.as_deref()),
        ]
        .into_iter()
        .filter_map(|(name, value)| {
            value
                .filter(|value| !value.trim().is_empty())
                .is_none()
                .then_some(name)
        })
        .collect::<Vec<_>>();

        if !missing.is_empty() {
            return Err(RivoraError::invalid_value(
                "slack_env",
                format!(
                    "missing {}; set SLACK_BOT_TOKEN, SLACK_APP_TOKEN, and SLACK_SIGNING_SECRET. No Slack tokens were stored.",
                    missing.join(", ")
                ),
            ));
        }

        Ok(Self {
            bot_token: bot_token.unwrap_or_default(),
            app_token: app_token.unwrap_or_default(),
            signing_secret: signing_secret.unwrap_or_default(),
        })
    }

    #[must_use]
    pub fn redact(&self, value: &str) -> String {
        redact_exact_secrets(
            &redact_slack_token_like_values(value),
            &[&self.bot_token, &self.app_token, &self.signing_secret],
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackAppMentionEvent {
    pub channel: String,
    pub user: String,
    pub text: String,
    pub timestamp: String,
    pub thread_ts: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SlackPostMessageRequest {
    pub channel: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_ts: Option<String>,
    pub unfurl_links: bool,
    pub unfurl_media: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SocketModeAcknowledgement {
    envelope_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SocketModeEnvelope {
    envelope_id: Option<String>,
    #[serde(rename = "type")]
    envelope_type: String,
    payload: Option<SocketModePayload>,
}

impl SocketModeEnvelope {
    fn acknowledgement(&self) -> Option<SocketModeAcknowledgement> {
        self.envelope_id
            .as_ref()
            .map(|envelope_id| SocketModeAcknowledgement {
                envelope_id: envelope_id.clone(),
            })
    }

    fn app_mention(&self) -> Option<SlackAppMentionEvent> {
        if self.envelope_type != "events_api" {
            return None;
        }
        let event = self.payload.as_ref()?.event.as_ref()?;
        if event.event_type != "app_mention" || event.bot_id.is_some() {
            return None;
        }
        let timestamp = event.timestamp.clone()?;
        Some(SlackAppMentionEvent {
            channel: event.channel.clone()?,
            user: event.user.clone()?,
            text: event.text.clone()?,
            timestamp: timestamp.clone(),
            thread_ts: event.thread_ts.clone().or(Some(timestamp)),
        })
    }

    fn requests_reconnect(&self) -> bool {
        self.envelope_type == "disconnect"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SocketModePayload {
    event: Option<SocketModeEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SocketModeEvent {
    #[serde(rename = "type")]
    event_type: String,
    user: Option<String>,
    text: Option<String>,
    #[serde(rename = "ts")]
    timestamp: Option<String>,
    channel: Option<String>,
    thread_ts: Option<String>,
    bot_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SocketLoopOutcome {
    Reconnect,
}

trait SlackSocketConnection {
    fn read_text(&mut self) -> Result<Option<String>>;
    fn send_text(&mut self, text: &str) -> Result<()>;
}

trait SlackWebApiClient {
    fn open_socket_url(&self, app_token: &str) -> Result<String>;
    fn post_message(&self, bot_token: &str, request: &SlackPostMessageRequest) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
struct CurlSlackWebApiClient;

#[derive(Debug, Deserialize)]
struct SlackApiResponse {
    ok: bool,
    url: Option<String>,
    error: Option<String>,
}

struct TungsteniteSlackSocket {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl SlackWebApiClient for CurlSlackWebApiClient {
    fn open_socket_url(&self, app_token: &str) -> Result<String> {
        let response = slack_api_post(app_token, SLACK_CONNECTIONS_OPEN_URL, "{}")?;
        let response: SlackApiResponse = serde_json::from_str(&response).map_err(|_| {
            RivoraError::provider(
                "slack",
                "Slack returned an invalid apps.connections.open response",
            )
        })?;
        if !response.ok {
            return Err(slack_api_error(
                "apps.connections.open",
                response.error.as_deref(),
                app_token,
            ));
        }
        response.url.ok_or_else(|| {
            RivoraError::provider("slack", "Slack did not return a Socket Mode WebSocket URL")
        })
    }

    fn post_message(&self, bot_token: &str, request: &SlackPostMessageRequest) -> Result<()> {
        let body = serde_json::to_string(request)?;
        let response = slack_api_post(bot_token, SLACK_POST_MESSAGE_URL, &body)?;
        let response: SlackApiResponse = serde_json::from_str(&response).map_err(|_| {
            RivoraError::provider(
                "slack",
                "Slack returned an invalid chat.postMessage response",
            )
        })?;
        if response.ok {
            Ok(())
        } else {
            Err(slack_api_error(
                "chat.postMessage",
                response.error.as_deref(),
                bot_token,
            ))
        }
    }
}

impl SlackSocketConnection for TungsteniteSlackSocket {
    fn read_text(&mut self) -> Result<Option<String>> {
        loop {
            let message = match self.socket.read() {
                Ok(message) => message,
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    return Ok(None)
                }
                Err(_) => {
                    return Err(RivoraError::provider(
                        "slack_socket",
                        "Slack Socket Mode connection read failed",
                    ))
                }
            };
            match message {
                Message::Text(text) => return Ok(Some(text.to_string())),
                Message::Close(_) => return Ok(None),
                Message::Ping(_) => {
                    self.socket.flush().map_err(|_| {
                        RivoraError::provider(
                            "slack_socket",
                            "Slack Socket Mode ping response failed",
                        )
                    })?;
                }
                Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }

    fn send_text(&mut self, text: &str) -> Result<()> {
        self.socket
            .send(Message::Text(text.to_string().into()))
            .map_err(|_| {
                RivoraError::provider("slack_socket", "Slack Socket Mode acknowledgement failed")
            })
    }
}

fn slack_api_post(token: &str, url: &str, body: &str) -> Result<String> {
    let config = slack_curl_config(token, url, body);
    let mut child = ProcessCommand::new("curl")
        .arg("--config")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| RivoraError::provider("slack", "curl is required for Slack Web API calls"))?;
    {
        let mut stdin = child.stdin.take().ok_or_else(|| {
            RivoraError::provider("slack", "could not open the curl configuration pipe")
        })?;
        stdin.write_all(config.as_bytes()).map_err(|_| {
            RivoraError::provider("slack", "could not write the curl configuration")
        })?;
    }
    let output = child
        .wait_with_output()
        .map_err(|_| RivoraError::provider("slack", "Slack Web API request did not finish"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let redacted =
            redact_exact_secrets(&redact_slack_token_like_values(stderr.as_ref()), &[token]);
        return Err(RivoraError::provider(
            "slack",
            format!("Slack Web API request failed: {}", redacted.trim()),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn slack_curl_config(token: &str, url: &str, body: &str) -> String {
    format!(
        "url = \"{}\"\nsilent\nshow-error\nfail\nrequest = \"POST\"\nheader = \"Authorization: Bearer {}\"\nheader = \"Content-Type: application/json; charset=utf-8\"\nheader = \"User-Agent: rivora-cli\"\ndata = \"{}\"\n",
        curl_config_escape(url),
        curl_config_escape(token),
        curl_config_escape(body)
    )
}

fn curl_config_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn slack_api_error(method: &'static str, error: Option<&str>, token: &str) -> RivoraError {
    let message = error.unwrap_or("unknown_error");
    let redacted = redact_exact_secrets(&redact_slack_token_like_values(message), &[token]);
    RivoraError::provider("slack", format!("{method} failed: {redacted}"))
}

fn connect_slack_socket(url: &str) -> Result<TungsteniteSlackSocket> {
    let (socket, _) = connect(url).map_err(|_| {
        RivoraError::provider(
            "slack_socket",
            "could not establish the Slack Socket Mode WebSocket connection",
        )
    })?;
    Ok(TungsteniteSlackSocket { socket })
}

pub fn run_slack_command(store: &LocalMemoryStore, command: SlackCommand) -> Result<String> {
    let store = slack_store_from_env(&store.root);
    match command {
        SlackCommand::Dev(options) => run_slack_dev(&store, options),
        SlackCommand::Doctor(options) => run_slack_doctor(&store, options),
        SlackCommand::Socket(_) => run_slack_socket(&store),
    }
}

pub fn run_slack_dev(store: &LocalMemoryStore, options: SlackDevOptions) -> Result<String> {
    if !store.store_dir().exists() {
        return Ok(slack_setup_guidance_no_store());
    }
    let response = route_slack_mention(
        store,
        &SlackAppMentionEvent {
            channel: options.channel,
            user: options.user,
            text: options.text,
            timestamp: DEFAULT_TIMESTAMP.to_string(),
            thread_ts: None,
        },
    )?;
    Ok(format!(
        "Rivora Slack dev response\n\n{}\n\nNo Slack tokens were used. No infrastructure actions were taken.",
        response
    ))
}

pub fn run_slack_doctor(store: &LocalMemoryStore, options: SlackDoctorOptions) -> Result<String> {
    let mut lines = vec!["Rivora Slack Doctor".to_string(), "".to_string()];

    lines.push("Environment:".to_string());
    let bot_token = std::env::var("SLACK_BOT_TOKEN").ok();
    let app_token = std::env::var("SLACK_APP_TOKEN").ok();
    let signing_secret = std::env::var("SLACK_SIGNING_SECRET").ok();
    let store_dir_override = std::env::var("RIVORA_STORE_DIR").ok();

    let bot_status = env_status(&bot_token);
    let app_status = env_status(&app_token);
    let signing_status = env_status(&signing_secret);
    lines.push(format!("- SLACK_BOT_TOKEN: {bot_status}"));
    lines.push(format!("- SLACK_APP_TOKEN: {app_status}"));
    lines.push(format!("- SLACK_SIGNING_SECRET: {signing_status}"));
    match store_dir_override.as_deref() {
        Some(value) if !value.trim().is_empty() => {
            lines.push(format!("- RIVORA_STORE_DIR: {value}"));
        }
        _ => {
            lines.push("- RIVORA_STORE_DIR: .rivora (default)".to_string());
        }
    }

    lines.push("".to_string());
    lines.push("Local store:".to_string());
    let store_dir = store.store_dir();
    if store_dir.exists() {
        lines.push(format!("- {}/: found", display_path(&store_dir)));
        check_store_file(&mut lines, &store.memories_path(), "memories.json");
        check_store_file(&mut lines, &store.evidence_path(), "evidence.json");
        check_store_file(&mut lines, &store.feedback_path(), "feedback.json");
        check_store_file(&mut lines, &store.receipts_path(), "receipts.json");
    } else {
        lines.push(format!("- {}/: not found", display_path(&store_dir)));
        lines.push("".to_string());
        lines.push("Run:".to_string());
        lines.push("  rivora init".to_string());
        lines.push("  rivora ingest git --repo . --limit 20".to_string());
        lines.push("".to_string());
        lines.push("Then:".to_string());
        lines.push("  rivora slack socket".to_string());
    }

    lines.push("".to_string());
    lines.push("Slack mode:".to_string());
    lines.push("- Socket Mode command: available".to_string());

    lines.push("".to_string());
    lines.push("No tokens were printed.".to_string());
    lines.push("No infrastructure actions were taken.".to_string());

    if options.live {
        let all_tokens_present = bot_token.is_some()
            && app_token.is_some()
            && signing_secret.is_some()
            && bot_token.as_deref().is_some_and(|v| !v.trim().is_empty())
            && app_token.as_deref().is_some_and(|v| !v.trim().is_empty())
            && signing_secret
                .as_deref()
                .is_some_and(|v| !v.trim().is_empty());
        if all_tokens_present {
            let api = CurlSlackWebApiClient;
            let config = SlackTokenConfig::from_env()?;
            match api.open_socket_url(&config.app_token) {
                Ok(_) => {
                    lines.push("".to_string());
                    lines.push("Live check: apps.connections.open succeeded.".to_string());
                }
                Err(error) => {
                    lines.push("".to_string());
                    lines.push(format!("Live check failed: {error}"));
                }
            }
        } else {
            lines.push("".to_string());
            lines.push("Live check skipped: not all tokens are set.".to_string());
        }
    }

    Ok(lines.join("\n"))
}

fn env_status(value: &Option<String>) -> &'static str {
    match value {
        Some(v) if !v.trim().is_empty() => "set",
        _ => "not set",
    }
}

fn check_store_file(lines: &mut Vec<String>, path: &Path, name: &str) {
    if path.exists() {
        lines.push(format!("- {name}: found"));
    } else {
        lines.push(format!("- {name}: not found"));
    }
}

fn print_socket_startup_summary(store: &LocalMemoryStore) {
    println!("Starting Rivora self-hosted Slack adapter...");
    println!();
    println!("Mode: Socket Mode");
    println!("Store: {}", display_path(&store.store_dir()));
    if store.store_dir().exists() {
        if store.memories_path().exists() {
            println!("Memory store: found");
        } else {
            println!("Memory store: not found");
        }
        if store.evidence_path().exists() {
            println!("Evidence store: found");
        } else {
            println!("Evidence store: not found");
        }
    } else {
        println!("Memory store: not found");
        println!("Evidence store: not found");
    }
    println!("Slack tokens: loaded from environment");
    println!("Token output: redacted");
    println!();
    println!("Listening for app mentions. Press Ctrl-C to stop.");
    println!("No infrastructure actions will be taken.");
}

pub fn run_slack_socket(store: &LocalMemoryStore) -> Result<String> {
    let config = SlackTokenConfig::from_env()?;
    let api = CurlSlackWebApiClient;
    let mut announced = false;
    let mut reconnect_attempts;
    loop {
        let socket_url = api.open_socket_url(&config.app_token)?;
        let mut socket = connect_slack_socket(&socket_url)?;
        reconnect_attempts = 0;
        if !announced {
            print_socket_startup_summary(store);
            announced = true;
        }
        match process_socket_connection(store, &config, &mut socket, &api)? {
            SocketLoopOutcome::Reconnect => {
                reconnect_attempts += 1;
                if reconnect_attempts > SLACK_MAX_RECONNECT_ATTEMPTS {
                    return Err(RivoraError::provider(
                        "slack_socket",
                        format!(
                            "Slack Socket Mode reconnection failed after {} attempts. \
                             Check your network and Slack app configuration.",
                            SLACK_MAX_RECONNECT_ATTEMPTS
                        ),
                    ));
                }
                let backoff = SLACK_RECONNECT_DELAY
                    .mul_f64(2.0_f64.powi((reconnect_attempts as i32 - 1).min(5)))
                    .min(SLACK_RECONNECT_BACKOFF_MAX);
                eprintln!(
                    "Slack Socket Mode reconnecting (attempt {}/{})...",
                    reconnect_attempts, SLACK_MAX_RECONNECT_ATTEMPTS
                );
                thread::sleep(backoff);
            }
        }
    }
}

fn process_socket_connection(
    store: &LocalMemoryStore,
    config: &SlackTokenConfig,
    socket: &mut dyn SlackSocketConnection,
    api: &dyn SlackWebApiClient,
) -> Result<SocketLoopOutcome> {
    let mut seen_envelope_ids = HashSet::new();
    loop {
        let Some(raw) = socket.read_text()? else {
            return Ok(SocketLoopOutcome::Reconnect);
        };
        let envelope = match parse_socket_mode_envelope(&raw) {
            Ok(envelope) => envelope,
            Err(_) => continue,
        };
        if let Some(acknowledgement) = envelope.acknowledgement() {
            socket.send_text(&serde_json::to_string(&acknowledgement)?)?;
        }
        if envelope.requests_reconnect() {
            return Ok(SocketLoopOutcome::Reconnect);
        }
        if let Some(envelope_id) = &envelope.envelope_id {
            if !seen_envelope_ids.insert(envelope_id.clone()) {
                continue;
            }
        }
        if let Some(event) = envelope.app_mention() {
            let response = route_slack_mention(store, &event)?;
            let request = build_post_message_request(config, &event, &response);
            api.post_message(&config.bot_token, &request)?;
        }
    }
}

fn parse_socket_mode_envelope(raw: &str) -> Result<SocketModeEnvelope> {
    serde_json::from_str(raw).map_err(|_| {
        RivoraError::provider("slack_socket", "Slack sent an invalid Socket Mode envelope")
    })
}

fn route_slack_mention(store: &LocalMemoryStore, event: &SlackAppMentionEvent) -> Result<String> {
    if !store.store_dir().exists() {
        return Ok(slack_setup_guidance_no_store());
    }
    let normalized = normalize_app_mention_text(&event.text, None);
    let snapshot = store.load()?;
    let mention = build_mention_request(&normalized, event, &snapshot);
    let _ = SlackReliabilityMemoryApp::new().handle_mention(mention)?;
    ask(store, &normalized)
}

fn build_post_message_request(
    config: &SlackTokenConfig,
    event: &SlackAppMentionEvent,
    response: &str,
) -> SlackPostMessageRequest {
    let redacted = config.redact(response);
    SlackPostMessageRequest {
        channel: event.channel.clone(),
        text: truncate_slack_text(&redacted),
        thread_ts: event.thread_ts.clone(),
        unfurl_links: false,
        unfurl_media: false,
    }
}

fn truncate_slack_text(value: &str) -> String {
    value.chars().take(SLACK_MESSAGE_LIMIT).collect()
}

pub fn handle_slack_feedback_action(
    store: &LocalMemoryStore,
    action: SlackFeedbackActionKind,
    memory_id: &str,
    actor_id: &str,
    channel_id: &str,
    note: Option<String>,
    correction_text: Option<String>,
) -> Result<String> {
    if !store.store_dir().exists() {
        return Ok(slack_setup_guidance_no_store());
    }

    if action == SlackFeedbackActionKind::Correct && note.is_none() && correction_text.is_none() {
        return Ok(format!(
            "Correction flow is not available in self-hosted Slack yet.\n\nUse:\nrivora feedback {memory_id} correct --note \"...\"\n\nNo infrastructure actions were taken."
        ));
    }

    let mut snapshot = store.load()?;
    let index = snapshot
        .memories
        .iter()
        .position(|memory| memory.id.as_str() == memory_id)
        .ok_or_else(|| RivoraError::invalid_value("memory_id", "memory was not found"))?;

    let target_memory = snapshot.memories[index].clone();
    let slack_action = SlackFeedbackAction {
        action,
        actor_id: actor_id.to_string(),
        channel_id: channel_id.to_string(),
        timestamp: DEFAULT_TIMESTAMP.to_string(),
        target_memory,
        note,
        correction_text,
    };
    let feedback = slack_action.to_human_feedback()?;
    let response = SlackReliabilityMemoryApp::new().handle_feedback_action(slack_action)?;

    let mut updated = response.updated_memory;
    updated.receipt_ids.extend(
        response
            .receipts
            .iter()
            .map(|receipt| receipt.id.as_str().to_string()),
    );
    snapshot.memories[index] = updated.clone();
    store.save_memories(&snapshot.memories)?;
    store.append_feedback(feedback)?;
    store.append_receipts(response.receipts)?;

    Ok(render_feedback_response(&updated))
}

#[must_use]
pub fn normalize_app_mention_text(text: &str, bot_user_id: Option<&str>) -> String {
    let mut normalized = text.trim().to_string();
    if let Some(bot_user_id) = bot_user_id {
        let mention = format!("<@{bot_user_id}>");
        normalized = normalized.replace(&mention, "");
    }
    while let Some(rest) = normalized.trim_start().strip_prefix("<@") {
        if let Some((_, tail)) = rest.split_once('>') {
            normalized = tail.trim_start().to_string();
        } else {
            break;
        }
    }
    normalized
        .trim_start_matches("@rivora")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
pub fn redact_slack_token_like_values(value: &str) -> String {
    value
        .split_whitespace()
        .map(|part| {
            let trimmed = part.trim_matches(|c: char| {
                !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            });
            if ["xoxb-", "xapp-", "xoxp-", "xoxa-"]
                .iter()
                .any(|prefix| trimmed.starts_with(prefix))
            {
                part.replace(trimmed, "[redacted]")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
pub fn slack_output_contains_infrastructure_action(output: &str) -> bool {
    crate::output_contains_infrastructure_action(output)
}

fn build_mention_request(
    text: &str,
    event: &SlackAppMentionEvent,
    snapshot: &StoreSnapshot,
) -> SlackMentionRequest {
    let topic = first_meaningful_topic(text);
    SlackMentionRequest {
        channel_id: event.channel.clone(),
        user_id: event.user.clone(),
        text: text.to_string(),
        timestamp: event.timestamp.clone(),
        thread_ts: event.thread_ts.clone(),
        service: topic.clone(),
        topic,
        evidence_ids: snapshot
            .evidence
            .iter()
            .take(3)
            .map(|item| item.id.as_str().to_string())
            .collect(),
        memory_records: snapshot.memories.clone(),
    }
}

fn slack_store_from_env(cwd: &Path) -> LocalMemoryStore {
    match std::env::var("RIVORA_STORE_DIR") {
        Ok(value) if !value.trim().is_empty() => {
            LocalMemoryStore::with_store_dir(cwd, PathBuf::from(value))
        }
        _ => LocalMemoryStore::new(cwd),
    }
}

fn redact_exact_secrets(value: &str, secrets: &[&str]) -> String {
    secrets.iter().fold(value.to_string(), |redacted, secret| {
        if secret.is_empty() {
            redacted
        } else {
            redacted.replace(*secret, "[redacted]")
        }
    })
}

fn slack_setup_guidance_no_store() -> String {
    "Rivora is not initialized yet.\n\nRun:\nrivora init\nrivora ingest git --repo . --limit 20\n\nThen ask again:\n@rivora what changed?\n\nNo infrastructure actions were taken."
        .to_string()
}

#[allow(dead_code)]
fn slack_setup_guidance_no_evidence() -> String {
    "Rivora does not have evidence yet.\n\nTry:\nrivora ingest git --repo . --limit 20\nrivora ingest github --repo owner/name --limit 20\n\nEvidence is not memory until your team approves it.\nNo infrastructure actions were taken."
        .to_string()
}

#[allow(dead_code)]
fn slack_setup_guidance_no_approved_memories() -> String {
    "No approved memories found yet.\n\nEvidence is not memory until approved.\n\nTry:\nrivora remember --from-evidence <evidence-id>\nrivora feedback <memory-id> approve\n\nNo infrastructure actions were taken."
        .to_string()
}

#[allow(dead_code)]
fn slack_unknown_prompt_guidance() -> String {
    "Try asking:\n\n@rivora what changed?\n@rivora what merged recently?\n@rivora what failed recently?\n@rivora have we seen checkout latency before?\n@rivora recall checkout\n\nNo infrastructure actions were taken."
        .to_string()
}

fn render_feedback_response(memory: &MemoryRecord) -> String {
    format!(
        "Slack feedback recorded.\n\nMemory: {}\nStatus: {}\n\nNo infrastructure actions were taken.",
        memory.id.as_str(),
        status_label(memory.status)
    )
}

fn status_label(status: MemoryStatus) -> &'static str {
    match status {
        MemoryStatus::Candidate => "Candidate",
        MemoryStatus::Active => "Active",
        MemoryStatus::Rejected => "Rejected",
        MemoryStatus::Corrected => "Corrected",
        MemoryStatus::Superseded => "Superseded",
        MemoryStatus::Expired => "Expired",
        MemoryStatus::Archived => "Archived",
        MemoryStatus::Invalid => "Invalid",
        MemoryStatus::Draft => "Draft",
    }
}

fn first_meaningful_topic(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-'))
        .find(|token| {
            !token.is_empty()
                && !matches!(
                    token.to_ascii_lowercase().as_str(),
                    "what"
                        | "changed"
                        | "merged"
                        | "failed"
                        | "recently"
                        | "have"
                        | "seen"
                        | "this"
                        | "before"
                        | "recall"
                        | "should"
                        | "remember"
                )
        })
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{run, EvidenceItem, LocalMemoryStore};
    use rivora_adaptive::{AdaptiveMemoryEngine, MemoryCandidateRequest};
    use rivora_connectors::{EvidenceId, EvidenceKind, EvidenceSource};
    use rivora_memory::{MemoryKind, MemoryScope};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use tempfile::TempDir;

    const SLACK_SOURCE: &str = "self-hosted-slack";
    const APP_MENTION_ENVELOPE: &str = r#"{
        "envelope_id": "env-123",
        "type": "events_api",
        "accepts_response_payload": false,
        "payload": {
            "type": "event_callback",
            "event_id": "Ev123",
            "event": {
                "type": "app_mention",
                "user": "U123",
                "text": "<@UBOT> what changed?",
                "ts": "1710000000.000100",
                "channel": "C123"
            }
        }
    }"#;

    fn temp_store() -> (TempDir, LocalMemoryStore) {
        let temp = TempDir::new().unwrap();
        let store = LocalMemoryStore::new(temp.path());
        (temp, store)
    }

    fn checkout_evidence() -> EvidenceItem {
        EvidenceItem {
            id: EvidenceId::new("github:pr:owner/name:128").unwrap(),
            kind: EvidenceKind::GitHubPullRequestMerged,
            source: EvidenceSource::github("owner/name"),
            title: "PR #128: Reduce checkout worker concurrency".to_string(),
            summary: "PR #128 merged: \"Reduce checkout worker concurrency\"".to_string(),
            body: "Repository: owner/name\nLabels: service:checkout".to_string(),
            service: Some("checkout".to_string()),
            files_changed: Vec::new(),
            timestamp: Some("2026-06-28T00:00:00Z".to_string()),
            author: Some("ada".to_string()),
            tags: vec!["checkout".to_string()],
            refs: vec!["pr:128".to_string()],
            confidence: 0.85,
        }
    }

    fn workflow_failure_evidence() -> EvidenceItem {
        EvidenceItem {
            id: EvidenceId::new("github:workflow:owner/name:1001").unwrap(),
            kind: EvidenceKind::GitHubWorkflowFailed,
            source: EvidenceSource::github("owner/name"),
            title: "Workflow failed: checkout smoke test".to_string(),
            summary: "Checkout smoke test failed on main".to_string(),
            body: "Conclusion: failure. No root cause was inferred.".to_string(),
            service: Some("checkout".to_string()),
            files_changed: Vec::new(),
            timestamp: Some("2026-06-28T00:00:00Z".to_string()),
            author: Some("github-actions".to_string()),
            tags: vec!["checkout".to_string(), "workflow".to_string()],
            refs: vec!["workflow:1001".to_string()],
            confidence: 0.82,
        }
    }

    struct FakeSocket {
        messages: VecDeque<String>,
        sent: Vec<String>,
        events: Rc<RefCell<Vec<&'static str>>>,
    }

    impl SlackSocketConnection for FakeSocket {
        fn read_text(&mut self) -> Result<Option<String>> {
            Ok(self.messages.pop_front())
        }

        fn send_text(&mut self, text: &str) -> Result<()> {
            self.events.borrow_mut().push("ack");
            self.sent.push(text.to_string());
            Ok(())
        }
    }

    struct FakeSlackApi {
        posts: RefCell<Vec<SlackPostMessageRequest>>,
        events: Rc<RefCell<Vec<&'static str>>>,
    }

    impl SlackWebApiClient for FakeSlackApi {
        fn open_socket_url(&self, _app_token: &str) -> Result<String> {
            Ok("wss://socket.example.invalid/link".to_string())
        }

        fn post_message(&self, _bot_token: &str, request: &SlackPostMessageRequest) -> Result<()> {
            self.events.borrow_mut().push("post");
            self.posts.borrow_mut().push(request.clone());
            Ok(())
        }
    }

    fn active_memory(id: &str) -> MemoryRecord {
        let mut memory = AdaptiveMemoryEngine::new()
            .propose_candidate(MemoryCandidateRequest {
                id: id.to_string(),
                kind: MemoryKind::IncidentLearning,
                scope: MemoryScope::Service,
                service: "checkout".to_string(),
                symptoms: vec!["latency".to_string()],
                event_summary: "checkout latency increased after worker change".to_string(),
                evidence_ids: vec!["github:pr:owner/name:128".to_string()],
                source: "github".to_string(),
                source_version: "0.1.0".to_string(),
                confidence: 0.85,
                observed_at: DEFAULT_TIMESTAMP.to_string(),
                learned_at: DEFAULT_TIMESTAMP.to_string(),
            })
            .unwrap()
            .memory;
        memory.approve();
        memory
    }

    #[test]
    fn token_redaction_covers_slack_tokens_and_exact_secret() {
        let config = SlackTokenConfig::from_values(
            Some("xoxb-secret-bot".to_string()),
            Some("xapp-secret-app".to_string()),
            Some("plain-signing-secret".to_string()),
        )
        .unwrap();
        let message =
            "failed xoxb-secret-bot xapp-secret-app and plain-signing-secret should be hidden";

        let redacted = config.redact(message);

        assert!(!redacted.contains("xoxb-secret-bot"));
        assert!(!redacted.contains("xapp-secret-app"));
        assert!(!redacted.contains("plain-signing-secret"));
        assert!(redacted.contains("[redacted]"));
        let debug = format!("{config:?}");
        assert!(!debug.contains("xoxb-secret-bot"));
        assert!(!debug.contains("xapp-secret-app"));
        assert!(!debug.contains("plain-signing-secret"));
    }

    #[test]
    fn missing_env_vars_produce_safe_setup_error() {
        let error = SlackTokenConfig::from_values(None, Some("xapp-token".to_string()), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("SLACK_BOT_TOKEN"));
        assert!(error.contains("SLACK_SIGNING_SECRET"));
        assert!(!error.contains("xapp-token"));
    }

    #[test]
    fn app_mention_text_normalization_removes_bot_mentions() {
        assert_eq!(
            normalize_app_mention_text(
                "<@U123> have we seen checkout latency before?",
                Some("U123")
            ),
            "have we seen checkout latency before?"
        );
        assert_eq!(
            normalize_app_mention_text("@rivora what changed?", None),
            "what changed?"
        );
    }

    #[test]
    fn app_mention_envelope_parses_and_builds_ack() {
        let envelope = parse_socket_mode_envelope(APP_MENTION_ENVELOPE).unwrap();
        let event = envelope.app_mention().unwrap();
        let ack = envelope.acknowledgement().unwrap();

        assert_eq!(envelope.envelope_id.as_deref(), Some("env-123"));
        assert_eq!(event.channel, "C123");
        assert_eq!(event.user, "U123");
        assert_eq!(event.text, "<@UBOT> what changed?");
        assert_eq!(event.thread_ts.as_deref(), Some("1710000000.000100"));
        assert_eq!(ack.envelope_id, "env-123");
        assert_eq!(
            serde_json::to_string(&ack).unwrap(),
            r#"{"envelope_id":"env-123"}"#
        );
    }

    #[test]
    fn non_mention_envelope_is_acknowledged_but_not_routed() {
        let raw = APP_MENTION_ENVELOPE.replace("app_mention", "message");
        let envelope = parse_socket_mode_envelope(&raw).unwrap();

        assert!(envelope.acknowledgement().is_some());
        assert!(envelope.app_mention().is_none());
    }

    #[test]
    fn empty_store_produces_setup_guidance() {
        let (_temp, store) = temp_store();
        let output = run_slack_dev(
            &store,
            SlackDevOptions {
                text: "what changed?".to_string(),
                channel: "C123".to_string(),
                user: "U123".to_string(),
                bot_user_id: None,
            },
        )
        .unwrap();

        assert!(output.contains("Rivora is not initialized yet."));
        assert!(output.contains("rivora init"));
    }

    #[test]
    fn dev_mode_routes_mention_to_what_changed_evidence() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store.append_evidence([checkout_evidence()]).unwrap();

        let output = run(
            [
                "slack",
                "dev",
                "--text",
                "<@U123> what changed in checkout?",
            ],
            &store.root,
        )
        .unwrap();

        assert!(output.contains("Rivora Slack dev response"));
        assert!(output.contains("github:pr:owner/name:128"));
        assert!(output.contains("No root cause was inferred."));
    }

    #[test]
    fn dev_mode_routes_app_mention_to_recall() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store
            .save_memories(&[active_memory("mem-checkout-latency")])
            .unwrap();

        let output = run(
            [
                "slack",
                "dev",
                "--text",
                "@rivora have we seen checkout latency before?",
            ],
            &store.root,
        )
        .unwrap();

        assert!(output.contains("Similar memories found: 1"));
        assert!(output.contains("mem-checkout-latency"));
    }

    #[test]
    fn adapter_reads_local_memory_and_evidence_stores() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store
            .save_memories(&[active_memory("mem-checkout-store")])
            .unwrap();
        store.append_evidence([checkout_evidence()]).unwrap();

        let recall = run(
            ["slack", "dev", "--text", "recall checkout latency"],
            &store.root,
        )
        .unwrap();
        let changed = run(
            ["slack", "dev", "--text", "what merged recently?"],
            &store.root,
        )
        .unwrap();

        assert!(recall.contains("mem-checkout-store"));
        assert!(changed.contains("github:pr:owner/name:128"));
    }

    #[test]
    fn app_mentions_route_to_github_merge_and_failure_evidence() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store
            .append_evidence([checkout_evidence(), workflow_failure_evidence()])
            .unwrap();

        let merged = route_slack_mention(
            &store,
            &SlackAppMentionEvent {
                channel: "C123".to_string(),
                user: "U123".to_string(),
                text: "<@UBOT> what merged recently?".to_string(),
                timestamp: "1710000000.000100".to_string(),
                thread_ts: None,
            },
        )
        .unwrap();
        let failed = route_slack_mention(
            &store,
            &SlackAppMentionEvent {
                channel: "C123".to_string(),
                user: "U123".to_string(),
                text: "<@UBOT> what failed recently?".to_string(),
                timestamp: "1710000000.000200".to_string(),
                thread_ts: None,
            },
        )
        .unwrap();

        assert!(merged.contains("github:pr:owner/name:128"));
        assert!(failed.contains("github:workflow:owner/name:1001"));
        assert!(failed.contains("No root cause was inferred."));
    }

    #[test]
    fn initialized_store_without_evidence_returns_ingest_guidance() {
        let (_temp, store) = temp_store();
        store.init().unwrap();

        let response = route_slack_mention(
            &store,
            &SlackAppMentionEvent {
                channel: "C123".to_string(),
                user: "U123".to_string(),
                text: "<@UBOT> what changed?".to_string(),
                timestamp: "1710000000.000100".to_string(),
                thread_ts: None,
            },
        )
        .unwrap();

        assert!(response.contains("No evidence found yet."));
        assert!(response.contains("rivora ingest git"));
        assert!(response.contains("rivora ingest github"));
    }

    #[test]
    fn socket_connection_acknowledges_before_sending_response() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store.append_evidence([checkout_evidence()]).unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut socket = FakeSocket {
            messages: VecDeque::from([APP_MENTION_ENVELOPE.to_string()]),
            sent: Vec::new(),
            events: Rc::clone(&events),
        };
        let api = FakeSlackApi {
            posts: RefCell::new(Vec::new()),
            events: Rc::clone(&events),
        };
        let config = SlackTokenConfig::from_values(
            Some("xoxb-test-bot".to_string()),
            Some("xapp-test-app".to_string()),
            Some("test-signing-secret".to_string()),
        )
        .unwrap();

        let outcome = process_socket_connection(&store, &config, &mut socket, &api).unwrap();

        assert_eq!(outcome, SocketLoopOutcome::Reconnect);
        assert_eq!(&*events.borrow(), &["ack", "post"]);
        assert_eq!(socket.sent.len(), 1);
        assert!(socket.sent[0].contains("env-123"));
        let posts = api.posts.borrow();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].channel, "C123");
        assert_eq!(posts[0].thread_ts.as_deref(), Some("1710000000.000100"));
        assert!(posts[0].text.contains("github:pr:owner/name:128"));
        for path in [
            store.memories_path(),
            store.feedback_path(),
            store.receipts_path(),
            store.evidence_path(),
        ] {
            let raw = std::fs::read_to_string(path).unwrap();
            assert!(!raw.contains("xoxb-test-bot"));
            assert!(!raw.contains("xapp-test-app"));
            assert!(!raw.contains("test-signing-secret"));
        }
    }

    #[test]
    fn slack_send_payload_redacts_token_values() {
        let config = SlackTokenConfig::from_values(
            Some("xoxb-secret-bot".to_string()),
            Some("xapp-secret-app".to_string()),
            Some("plain-signing-secret".to_string()),
        )
        .unwrap();
        let request = build_post_message_request(
            &config,
            &SlackAppMentionEvent {
                channel: "C123".to_string(),
                user: "U123".to_string(),
                text: "what changed?".to_string(),
                timestamp: "1710000000.000100".to_string(),
                thread_ts: None,
            },
            "xoxb-secret-bot xapp-secret-app plain-signing-secret",
        );
        let serialized = serde_json::to_string(&request).unwrap();

        assert!(!serialized.contains("xoxb-secret-bot"));
        assert!(!serialized.contains("xapp-secret-app"));
        assert!(!serialized.contains("plain-signing-secret"));
        assert!(serialized.contains("[redacted]"));
    }

    #[test]
    fn feedback_action_updates_local_memory_state() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store
            .save_memories(&[AdaptiveMemoryEngine::new()
                .propose_candidate(MemoryCandidateRequest {
                    id: "mem-candidate".to_string(),
                    kind: MemoryKind::OperationalNote,
                    scope: MemoryScope::Service,
                    service: "checkout".to_string(),
                    symptoms: vec!["latency".to_string()],
                    event_summary: "checkout latency needs review".to_string(),
                    evidence_ids: vec!["github:pr:owner/name:128".to_string()],
                    source: SLACK_SOURCE.to_string(),
                    source_version: "0.1.0".to_string(),
                    confidence: 0.6,
                    observed_at: DEFAULT_TIMESTAMP.to_string(),
                    learned_at: DEFAULT_TIMESTAMP.to_string(),
                })
                .unwrap()
                .memory])
            .unwrap();

        let output = handle_slack_feedback_action(
            &store,
            SlackFeedbackActionKind::Remember,
            "mem-candidate",
            "U123",
            "C123",
            Some("team approved".to_string()),
            None,
        )
        .unwrap();
        let snapshot = store.load().unwrap();

        assert!(output.contains("Status: Active"));
        assert_eq!(snapshot.memories[0].status, MemoryStatus::Active);
        assert_eq!(snapshot.feedback.len(), 1);
        assert!(!snapshot.receipts.is_empty());
    }

    #[test]
    fn correct_action_without_note_returns_placeholder() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store
            .save_memories(&[active_memory("mem-correct")])
            .unwrap();

        let output = handle_slack_feedback_action(
            &store,
            SlackFeedbackActionKind::Correct,
            "mem-correct",
            "U123",
            "C123",
            None,
            None,
        )
        .unwrap();

        assert!(output.contains("Correction flow is not available"));
        assert!(output.contains("rivora feedback mem-correct correct"));
        assert_eq!(store.load().unwrap().feedback.len(), 0);
    }

    #[test]
    fn socket_configuration_validation_and_redaction_are_safe() {
        let missing = SlackTokenConfig::from_values(None, None, None)
            .unwrap_err()
            .to_string();
        assert!(missing.contains("missing"));
        assert!(!missing.contains("xox"));

        let configured = SlackTokenConfig::from_values(
            Some("xoxb-token".to_string()),
            Some("xapp-token".to_string()),
            Some("signing-secret".to_string()),
        )
        .unwrap()
        .redact("xoxb-token xapp-token signing-secret");
        assert_eq!(configured, "[redacted] [redacted] [redacted]");
    }

    #[test]
    fn slack_adapter_never_emits_infrastructure_mutation_actions_or_tokens() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store.append_evidence([checkout_evidence()]).unwrap();

        let outputs = [
            run(["slack", "dev", "--text", "what changed?"], &store.root).unwrap(),
            run(
                ["slack", "dev", "--text", "what failed recently?"],
                &store.root,
            )
            .unwrap(),
            run(
                ["slack", "dev", "--text", "what merged recently?"],
                &store.root,
            )
            .unwrap(),
        ];

        for output in outputs {
            assert!(!slack_output_contains_infrastructure_action(&output));
            assert!(!output.contains("xoxb-"));
            assert!(!output.contains("xapp-"));
        }
    }

    #[test]
    fn slack_tokens_are_not_stored_by_dev_mode() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store.append_evidence([checkout_evidence()]).unwrap();
        let output = run(["slack", "dev", "--text", "what changed?"], &store.root).unwrap();

        assert!(!output.contains("xoxb-"));
        for path in [
            store.memories_path(),
            store.feedback_path(),
            store.receipts_path(),
            store.evidence_path(),
        ] {
            let raw = std::fs::read_to_string(path).unwrap();
            assert!(!raw.contains("xoxb-"));
            assert!(!raw.contains("xapp-"));
            assert!(!raw.contains("SLACK_SIGNING_SECRET"));
        }
    }

    #[test]
    fn doctor_with_all_env_vars_present() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store.append_evidence([checkout_evidence()]).unwrap();
        std::env::set_var("SLACK_BOT_TOKEN", "xoxb-test-token");
        std::env::set_var("SLACK_APP_TOKEN", "xapp-test-token");
        std::env::set_var("SLACK_SIGNING_SECRET", "test-signing-secret");
        let result = run_slack_doctor(&store, SlackDoctorOptions { live: false }).unwrap();
        std::env::remove_var("SLACK_BOT_TOKEN");
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_SIGNING_SECRET");

        assert!(result.contains("Rivora Slack Doctor"));
        assert!(result.contains("SLACK_BOT_TOKEN: set"));
        assert!(result.contains("SLACK_APP_TOKEN: set"));
        assert!(result.contains("SLACK_SIGNING_SECRET: set"));
        assert!(result.contains("memories.json: found"));
        assert!(result.contains("evidence.json: found"));
        assert!(result.contains("Socket Mode command: available"));
        assert!(result.contains("No tokens were printed."));
        assert!(result.contains("No infrastructure actions were taken."));
    }

    #[test]
    fn doctor_with_missing_env_vars() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        std::env::remove_var("SLACK_BOT_TOKEN");
        std::env::remove_var("SLACK_APP_TOKEN");
        std::env::remove_var("SLACK_SIGNING_SECRET");
        let result = run_slack_doctor(&store, SlackDoctorOptions { live: false }).unwrap();

        assert!(result.contains("Rivora Slack Doctor"));
        assert!(result.contains("SLACK_BOT_TOKEN: not set"));
        assert!(result.contains("SLACK_APP_TOKEN: not set"));
        assert!(result.contains("SLACK_SIGNING_SECRET: not set"));
    }

    #[test]
    fn doctor_with_missing_local_store() {
        let (_temp, store) = temp_store();
        let result = run_slack_doctor(&store, SlackDoctorOptions { live: false }).unwrap();

        assert!(result.contains("Rivora Slack Doctor"));
        assert!(result.contains("not found"));
        assert!(result.contains("rivora init"));
        assert!(result.contains("rivora ingest git"));
    }

    #[test]
    fn doctor_output_redacts_token_values() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        let config = SlackTokenConfig::from_values(
            Some("xoxb-secret-bot".to_string()),
            Some("xapp-secret-app".to_string()),
            Some("signing-secret".to_string()),
        )
        .unwrap();
        let redacted = config.redact("xoxb-secret-bot xapp-secret-app signing-secret");

        assert!(!redacted.contains("xoxb-secret-bot"));
        assert!(!redacted.contains("xapp-secret-app"));
        assert!(!redacted.contains("signing-secret"));
    }

    #[test]
    fn doctor_with_no_evidence_shows_found_after_init() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        let result = run_slack_doctor(&store, SlackDoctorOptions { live: false }).unwrap();

        assert!(result.contains("evidence.json: found"));
    }

    #[test]
    fn malformed_slack_envelope_does_not_panic() {
        let result = parse_socket_mode_envelope("not json");
        assert!(result.is_err());

        let result = parse_socket_mode_envelope(r#"{"type": "events_api"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn unsupported_slack_event_is_ignored_safely() {
        let raw = r#"{
            "envelope_id": "env-456",
            "type": "events_api",
            "payload": {
                "type": "event_callback",
                "event": {
                    "type": "reaction_added",
                    "user": "U123"
                }
            }
        }"#;
        let envelope = parse_socket_mode_envelope(raw).unwrap();

        assert!(envelope.acknowledgement().is_some());
        assert!(envelope.app_mention().is_none());
    }

    #[test]
    fn missing_text_field_in_event_produces_no_mention() {
        let raw = r#"{
            "envelope_id": "env-789",
            "type": "events_api",
            "payload": {
                "type": "event_callback",
                "event": {
                    "type": "app_mention",
                    "user": "U123",
                    "ts": "1710000000.000100",
                    "channel": "C123"
                }
            }
        }"#;
        let envelope = parse_socket_mode_envelope(raw).unwrap();

        assert!(envelope.acknowledgement().is_some());
        assert!(envelope.app_mention().is_none());
    }

    #[test]
    fn missing_channel_field_in_event_produces_no_mention() {
        let raw = r#"{
            "envelope_id": "env-999",
            "type": "events_api",
            "payload": {
                "type": "event_callback",
                "event": {
                    "type": "app_mention",
                    "user": "U123",
                    "text": "<@BOT> what changed?",
                    "ts": "1710000000.000100"
                }
            }
        }"#;
        let envelope = parse_socket_mode_envelope(raw).unwrap();

        assert!(envelope.acknowledgement().is_some());
        assert!(envelope.app_mention().is_none());
    }

    #[test]
    fn thread_timestamp_response_targeting() {
        let event = SlackAppMentionEvent {
            channel: "C123".to_string(),
            user: "U123".to_string(),
            text: "what changed?".to_string(),
            timestamp: "1710000000.000100".to_string(),
            thread_ts: Some("1710000000.000050".to_string()),
        };
        let config = SlackTokenConfig::from_values(
            Some("xoxb-test".to_string()),
            Some("xapp-test".to_string()),
            Some("test-secret".to_string()),
        )
        .unwrap();
        let request = build_post_message_request(&config, &event, "test response");

        assert_eq!(request.thread_ts.as_deref(), Some("1710000000.000050"));
        assert_eq!(request.channel, "C123");
    }

    #[test]
    fn setup_guidance_when_store_missing() {
        let (_temp, store) = temp_store();
        let response = run_slack_dev(
            &store,
            SlackDevOptions {
                text: "what changed?".to_string(),
                channel: "C123".to_string(),
                user: "U123".to_string(),
                bot_user_id: None,
            },
        )
        .unwrap();

        assert!(response.contains("Rivora is not initialized yet."));
        assert!(response.contains("rivora init"));
        assert!(response.contains("No infrastructure actions were taken."));
    }

    #[test]
    fn no_evidence_guidance_when_evidence_missing() {
        let (_temp, store) = temp_store();
        store.init().unwrap();

        let response = route_slack_mention(
            &store,
            &SlackAppMentionEvent {
                channel: "C123".to_string(),
                user: "U123".to_string(),
                text: "<@UBOT> what changed?".to_string(),
                timestamp: "1710000000.000100".to_string(),
                thread_ts: None,
            },
        )
        .unwrap();

        assert!(response.contains("No evidence found yet."));
        assert!(response.contains("rivora ingest git"));
        assert!(response.contains("rivora ingest github"));
        assert!(response.contains("No root cause was inferred."));
        assert!(response.contains("No infrastructure actions were taken."));
    }

    #[test]
    fn feedback_action_maps_to_human_feedback() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        let candidate = AdaptiveMemoryEngine::new()
            .propose_candidate(MemoryCandidateRequest {
                id: "mem-feedback-map".to_string(),
                kind: MemoryKind::OperationalNote,
                scope: MemoryScope::Service,
                service: "checkout".to_string(),
                symptoms: vec!["latency".to_string()],
                event_summary: "checkout latency needs review".to_string(),
                evidence_ids: vec!["github:pr:owner/name:128".to_string()],
                source: SLACK_SOURCE.to_string(),
                source_version: "0.1.0".to_string(),
                confidence: 0.6,
                observed_at: DEFAULT_TIMESTAMP.to_string(),
                learned_at: DEFAULT_TIMESTAMP.to_string(),
            })
            .unwrap()
            .memory;
        assert_eq!(candidate.status, MemoryStatus::Candidate);
        store.save_memories(&[candidate]).unwrap();

        let output = handle_slack_feedback_action(
            &store,
            SlackFeedbackActionKind::Reject,
            "mem-feedback-map",
            "U123",
            "C123",
            Some("wrong memory".to_string()),
            None,
        )
        .unwrap();

        assert!(output.contains("Status: Rejected"));
        assert!(output.contains("No infrastructure actions were taken."));
        let snapshot = store.load().unwrap();
        assert_eq!(snapshot.feedback.len(), 1);
        assert_eq!(snapshot.memories[0].status, MemoryStatus::Rejected);
    }

    #[test]
    fn correction_action_returns_cli_fallback_guidance() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store
            .save_memories(&[active_memory("mem-correction-fallback")])
            .unwrap();

        let output = handle_slack_feedback_action(
            &store,
            SlackFeedbackActionKind::Correct,
            "mem-correction-fallback",
            "U123",
            "C123",
            None,
            None,
        )
        .unwrap();

        assert!(output.contains("Correction flow is not available"));
        assert!(output.contains("rivora feedback mem-correction-fallback correct"));
        assert!(output.contains("No infrastructure actions were taken."));
    }

    #[test]
    fn slack_responses_do_not_include_forbidden_infrastructure_action_labels() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store.append_evidence([checkout_evidence()]).unwrap();
        store
            .save_memories(&[active_memory("mem-safety-check")])
            .unwrap();

        let outputs = [
            run_slack_dev(
                &store,
                SlackDevOptions {
                    text: "what changed?".to_string(),
                    channel: "C123".to_string(),
                    user: "U123".to_string(),
                    bot_user_id: None,
                },
            )
            .unwrap(),
            run_slack_dev(
                &store,
                SlackDevOptions {
                    text: "have we seen checkout latency before?".to_string(),
                    channel: "C123".to_string(),
                    user: "U123".to_string(),
                    bot_user_id: None,
                },
            )
            .unwrap(),
            run_slack_dev(
                &store,
                SlackDevOptions {
                    text: "what merged recently?".to_string(),
                    channel: "C123".to_string(),
                    user: "U123".to_string(),
                    bot_user_id: None,
                },
            )
            .unwrap(),
            run_slack_dev(
                &store,
                SlackDevOptions {
                    text: "what failed recently?".to_string(),
                    channel: "C123".to_string(),
                    user: "U123".to_string(),
                    bot_user_id: None,
                },
            )
            .unwrap(),
        ];

        for output in outputs {
            assert!(
                !slack_output_contains_infrastructure_action(&output),
                "output contains forbidden action: {output}"
            );
            assert!(!output.contains("rollback"));
            assert!(!output.contains("remediation"));
            assert!(!output.contains("deploy"));
            assert!(!output.contains("restart"));
            assert!(!output.contains("scale"));
        }
    }

    #[test]
    fn tokens_are_not_written_to_local_store_files() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store.append_evidence([checkout_evidence()]).unwrap();
        let _output = run_slack_dev(
            &store,
            SlackDevOptions {
                text: "what changed?".to_string(),
                channel: "C123".to_string(),
                user: "U123".to_string(),
                bot_user_id: None,
            },
        )
        .unwrap();

        for path in [
            store.memories_path(),
            store.feedback_path(),
            store.receipts_path(),
            store.evidence_path(),
        ] {
            let raw = std::fs::read_to_string(path).unwrap();
            assert!(!raw.contains("xoxb-"));
            assert!(!raw.contains("xapp-"));
            assert!(!raw.contains("SLACK_BOT_TOKEN"));
            assert!(!raw.contains("SLACK_APP_TOKEN"));
            assert!(!raw.contains("SLACK_SIGNING_SECRET"));
        }
    }

    #[test]
    fn duplicate_envelope_is_skipped() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store.append_evidence([checkout_evidence()]).unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut socket = FakeSocket {
            messages: VecDeque::from([
                APP_MENTION_ENVELOPE.to_string(),
                APP_MENTION_ENVELOPE.to_string(),
            ]),
            sent: Vec::new(),
            events: Rc::clone(&events),
        };
        let api = FakeSlackApi {
            posts: RefCell::new(Vec::new()),
            events: Rc::clone(&events),
        };
        let config = SlackTokenConfig::from_values(
            Some("xoxb-test-bot".to_string()),
            Some("xapp-test-app".to_string()),
            Some("test-signing-secret".to_string()),
        )
        .unwrap();

        let outcome = process_socket_connection(&store, &config, &mut socket, &api).unwrap();

        assert_eq!(outcome, SocketLoopOutcome::Reconnect);
        assert_eq!(socket.sent.len(), 2);
        let posts = api.posts.borrow();
        assert_eq!(posts.len(), 1);
    }

    #[test]
    fn doctor_parses_live_flag() {
        let parsed = crate::parse_command(&[
            "slack".to_string(),
            "doctor".to_string(),
            "--live".to_string(),
        ])
        .unwrap();
        match parsed {
            crate::Command::Slack(crate::SlackCommand::Doctor(options)) => {
                assert!(options.live);
            }
            other => panic!("expected slack doctor command, got {other:?}"),
        }
    }

    #[test]
    fn doctor_parses_without_live_flag() {
        let parsed = crate::parse_command(&["slack".to_string(), "doctor".to_string()]).unwrap();
        match parsed {
            crate::Command::Slack(crate::SlackCommand::Doctor(options)) => {
                assert!(!options.live);
            }
            other => panic!("expected slack doctor command, got {other:?}"),
        }
    }

    #[test]
    fn bot_id_filtered_from_app_mention() {
        let raw = r#"{
            "envelope_id": "env-bot-filter",
            "type": "events_api",
            "payload": {
                "type": "event_callback",
                "event": {
                    "type": "app_mention",
                    "user": "U123",
                    "text": "<@UBOT> what changed?",
                    "ts": "1710000000.000100",
                    "channel": "C123",
                    "bot_id": "B123"
                }
            }
        }"#;
        let envelope = parse_socket_mode_envelope(raw).unwrap();

        assert!(envelope.acknowledgement().is_some());
        assert!(envelope.app_mention().is_none());
    }

    #[test]
    fn disconnect_envelope_requests_reconnect() {
        let raw = r#"{
            "envelope_id": "env-disconnect",
            "type": "disconnect"
        }"#;
        let envelope = parse_socket_mode_envelope(raw).unwrap();

        assert!(envelope.requests_reconnect());
        assert!(envelope.acknowledgement().is_some());
    }
}
