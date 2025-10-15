use std::{collections::HashMap, time::Duration};

use once_cell::sync::Lazy;
use reqwest::Client;
use serenity::{
    builder::{
        CreateAttachment, CreateCommand, CreateCommandOption, CreateMessage, EditAttachments,
    },
    model::{
        application::{CommandDataOptionValue, CommandOptionType},
        channel::Message,
    },
    prelude::Context,
};
use std::sync::Mutex;
use url::Url;

const MAX_FILE_SIZE: usize = 10_000_000; // 10 MB
const MAX_MESSAGE_SIZE: usize = 1900; // for code block safety
const TIMEOUT_SECS: u64 = 5;
const COOLDOWN_SECS: u64 = 10;

static LAST_CALL: Lazy<Mutex<HashMap<u64, std::time::Instant>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub const NAME: &str = "get";
pub const DESCRIPTION: &str = "HTTP GET を実行します";

fn validate_url(input: &str) -> Result<Url, String> {
    match Url::parse(input) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Ok(url),
        _ => Err("URL は http(s):// で始まる必要があります".into()),
    }
}

fn parse_headers_json(s: &str) -> Result<HashMap<String, String>, String> {
    if s.trim().is_empty() {
        return Ok(HashMap::new());
    }
    let v: serde_json::Value =
        serde_json::from_str(s).map_err(|e| format!("ヘッダー JSON の解析に失敗: {e}"))?;
    match v {
        serde_json::Value::Object(map) => {
            let mut headers = HashMap::new();
            for (k, v) in map.into_iter() {
                if let Some(s) = v.as_str() {
                    headers.insert(k, s.to_string());
                } else {
                    headers.insert(k, v.to_string());
                }
            }
            Ok(headers)
        }
        _ => Err("ヘッダーは JSON オブジェクトである必要があります".into()),
    }
}

async fn http_get(
    url: &str,
    headers: HashMap<String, String>,
) -> Result<(Vec<u8>, String), String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("HTTP クライアント作成に失敗: {e}"))?;

    let mut req = client.get(url);
    for (k, v) in headers {
        req = req.header(&k, v);
    }
    let resp = req.send().await.map_err(|e| format!("HTTP エラー: {e}"))?;

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("レスポンス読み取りに失敗: {e}"))?
        .to_vec();

    Ok((bytes, content_type))
}

pub fn is_html(content_type: &str) -> bool {
    content_type.starts_with("text/html")
}

pub fn to_display_text(bytes: &[u8], content_type: &str) -> Option<String> {
    // Only try to display for text/* or application/json
    if is_html(content_type) {
        return None;
    }
    if content_type.starts_with("text/") || content_type.starts_with("application/json") {
        String::from_utf8(bytes.to_vec()).ok()
    } else {
        None
    }
}

async fn send_as_attachment(
    ctx: &Context,
    msg: &Message,
    bytes: Vec<u8>,
    filename: &str,
) -> serenity::Result<()> {
    if bytes.len() > MAX_FILE_SIZE {
        msg.channel_id
            .say(
                &ctx.http,
                "レスポンスが 10MB を超えたため添付できませんでした",
            )
            .await?;
        return Ok(());
    }

    let attachment = CreateAttachment::bytes(bytes, filename);
    msg.channel_id
        .send_message(
            &ctx.http,
            CreateMessage::new()
                .content("結果を添付ファイルとして送信します")
                .add_file(attachment),
        )
        .await?;
    Ok(())
}

// プレフィックス: !get <url> [--headers <json>]
pub async fn run(ctx: &Context, msg: &Message) -> serenity::Result<()> {
    // rate limit: decide under lock, then drop before await
    let mut need_cooldown_msg: Option<u64> = None;
    {
        let mut map = LAST_CALL.lock().unwrap();
        let now = std::time::Instant::now();
        if let Some(prev) = map.get(&msg.author.id.get()) {
            let elapsed = now.duration_since(*prev).as_secs();
            if elapsed < COOLDOWN_SECS {
                need_cooldown_msg = Some(COOLDOWN_SECS - elapsed);
            } else {
                map.insert(msg.author.id.get(), now);
            }
        } else {
            map.insert(msg.author.id.get(), now);
        }
    }
    if let Some(rem) = need_cooldown_msg {
        msg.channel_id
            .say(
                &ctx.http,
                format!("クールダウン中です。{}秒後に再試行してください", rem),
            )
            .await?;
        return Ok(());
    }
    let content = msg.content.trim();
    let rest = content
        .strip_prefix(super::PREFIX)
        .map(str::trim_start)
        .and_then(|s| s.strip_prefix(NAME))
        .map(str::trim)
        .unwrap_or("");

    if rest.is_empty() {
        msg.channel_id
            .say(&ctx.http, "使い方: !get <url> [--headers <json>]")
            .await?;
        return Ok(());
    }

    // parse: first token is url, optional --headers <json>
    let mut parts = rest.split_whitespace();
    let url = parts.next().unwrap_or("");

    if let Err(e) = validate_url(url) {
        msg.channel_id.say(&ctx.http, e).await?;
        return Ok(());
    }

    // Collect the remainder as potential --headers JSON (supports spaces in JSON by joining)
    let mut headers_json = String::new();
    let mut saw_flag = false;
    for p in parts {
        if !saw_flag {
            if p == "--headers" {
                saw_flag = true;
            }
            continue;
        }
        if !headers_json.is_empty() {
            headers_json.push(' ');
        }
        headers_json.push_str(p);
    }

    let headers = if saw_flag {
        match parse_headers_json(&headers_json) {
            Ok(h) => h,
            Err(e) => {
                msg.channel_id.say(&ctx.http, e).await?;
                return Ok(());
            }
        }
    } else {
        HashMap::new()
    };

    msg.channel_id.say(&ctx.http, "取得中…").await?;

    match http_get(url, headers).await {
        Ok((bytes, ct)) => {
            if let Some(mut s) = to_display_text(&bytes, &ct) {
                if s.len() > MAX_MESSAGE_SIZE {
                    // send as file
                    return send_as_attachment(ctx, msg, bytes, "response.txt").await;
                } else if ct.starts_with("application/json") {
                    // pretty print if possible
                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        s = serde_json::to_string_pretty(&json).unwrap_or(s);
                    }
                    let block = format!("```json\n{}\n```", s);
                    msg.channel_id.say(&ctx.http, block).await?;
                    Ok(())
                } else {
                    let block = format!("```\n{}\n```", s);
                    msg.channel_id.say(&ctx.http, block).await?;
                    Ok(())
                }
            } else if is_html(&ct) || bytes.len() > MAX_MESSAGE_SIZE {
                let filename = if is_html(&ct) {
                    "response.html"
                } else {
                    "response.txt"
                };
                send_as_attachment(ctx, msg, bytes, filename).await
            } else {
                // binary small but not displayable
                send_as_attachment(ctx, msg, bytes, "response.bin").await
            }
        }
        Err(e) => {
            msg.channel_id
                .say(&ctx.http, format!("エラー: {}", e))
                .await?;
            Ok(())
        }
    }
}

// スラッシュ実行: /get url:<url> headers:<json?>
pub async fn slash_execute(
    ctx: &Context,
    command: &serenity::model::application::CommandInteraction,
) -> serenity::Result<()> {
    // rate limit per user (no await while holding lock)
    let mut cooldown_remain: Option<u64> = None;
    {
        let mut map = LAST_CALL.lock().unwrap();
        let now = std::time::Instant::now();
        let uid = command.user.id.get();
        if let Some(prev) = map.get(&uid) {
            let elapsed = now.duration_since(*prev).as_secs();
            if elapsed < COOLDOWN_SECS {
                cooldown_remain = Some(COOLDOWN_SECS - elapsed);
            } else {
                map.insert(uid, now);
            }
        } else {
            map.insert(uid, now);
        }
    }
    if let Some(rem) = cooldown_remain {
        command
            .create_response(
                &ctx.http,
                serenity::builder::CreateInteractionResponse::Message(
                    serenity::builder::CreateInteractionResponseMessage::new().content(format!(
                        "クールダウン中です。{}秒後に再試行してください",
                        rem
                    )),
                ),
            )
            .await?;
        return Ok(());
    }
    let mut url: Option<String> = None;
    let mut headers_json: Option<String> = None;
    for opt in &command.data.options {
        match (opt.name.as_str(), &opt.value) {
            ("url", CommandDataOptionValue::String(s)) => url = Some(s.clone()),
            ("headers", CommandDataOptionValue::String(s)) => headers_json = Some(s.clone()),
            _ => {}
        }
    }

    let url = match url {
        Some(u) => u,
        None => {
            command
                .create_response(
                    &ctx.http,
                    serenity::builder::CreateInteractionResponse::Message(
                        serenity::builder::CreateInteractionResponseMessage::new()
                            .content("url が必要です"),
                    ),
                )
                .await?;
            return Ok(());
        }
    };

    if let Err(e) = validate_url(&url) {
        command
            .create_response(
                &ctx.http,
                serenity::builder::CreateInteractionResponse::Message(
                    serenity::builder::CreateInteractionResponseMessage::new().content(e),
                ),
            )
            .await?;
        return Ok(());
    }

    // acknowledge immediately (defer) to allow more than 3 seconds
    command
        .create_response(
            &ctx.http,
            serenity::builder::CreateInteractionResponse::Defer(
                serenity::builder::CreateInteractionResponseMessage::new(),
            ),
        )
        .await?;

    let headers = match headers_json {
        Some(s) if !s.trim().is_empty() => match parse_headers_json(&s) {
            Ok(h) => h,
            Err(e) => {
                command
                    .edit_response(
                        &ctx.http,
                        serenity::builder::EditInteractionResponse::new().content(e),
                    )
                    .await?;
                return Ok(());
            }
        },
        _ => HashMap::new(),
    };

    match http_get(&url, headers).await {
        Ok((bytes, ct)) => {
            if let Some(mut s) = to_display_text(&bytes, &ct) {
                if s.len() > MAX_MESSAGE_SIZE {
                    if bytes.len() > MAX_FILE_SIZE {
                        command
                            .edit_response(
                                &ctx.http,
                                serenity::builder::EditInteractionResponse::new()
                                    .content("レスポンスが 10MB を超えたため添付できませんでした"),
                            )
                            .await?;
                    } else {
                        let filename = if is_html(&ct) {
                            "response.html"
                        } else {
                            "response.txt"
                        };
                        let files =
                            EditAttachments::new().add(CreateAttachment::bytes(bytes, filename));
                        command
                            .edit_response(
                                &ctx.http,
                                serenity::builder::EditInteractionResponse::new()
                                    .content("結果が長いためファイルで送信します")
                                    .attachments(files),
                            )
                            .await?;
                    }
                } else if ct.starts_with("application/json") {
                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        s = serde_json::to_string_pretty(&json).unwrap_or(s);
                    }
                    let block = format!("```json\n{}\n```", s);
                    command
                        .edit_response(
                            &ctx.http,
                            serenity::builder::EditInteractionResponse::new().content(block),
                        )
                        .await?;
                } else {
                    let block = format!("```\n{}\n```", s);
                    command
                        .edit_response(
                            &ctx.http,
                            serenity::builder::EditInteractionResponse::new().content(block),
                        )
                        .await?;
                }
            } else if bytes.len() > MAX_FILE_SIZE {
                command
                    .edit_response(
                        &ctx.http,
                        serenity::builder::EditInteractionResponse::new()
                            .content("レスポンスが 10MB を超えたため添付できませんでした"),
                    )
                    .await?;
            } else {
                let filename = if is_html(&ct) {
                    "response.html"
                } else {
                    "response.txt"
                };
                let files = EditAttachments::new().add(CreateAttachment::bytes(bytes, filename));
                command
                    .edit_response(
                        &ctx.http,
                        serenity::builder::EditInteractionResponse::new()
                            .content("結果をファイルで送信します")
                            .attachments(files),
                    )
                    .await?;
            }
            Ok(())
        }
        Err(e) => {
            command
                .edit_response(
                    &ctx.http,
                    serenity::builder::EditInteractionResponse::new()
                        .content(format!("エラー: {}", e)),
                )
                .await?;
            Ok(())
        }
    }
}

pub fn slash_register() -> CreateCommand {
    CreateCommand::new(NAME)
        .description(DESCRIPTION)
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "url", "取得先URL").required(true),
        )
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "headers",
            "JSON 形式のヘッダー (任意)",
        ))
}
