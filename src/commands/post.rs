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
use std::collections::HashMap as StdHashMap;
use std::sync::Mutex;
use url::Url;

const MAX_FILE_SIZE: usize = 10_000_000; // 10 MB
const MAX_MESSAGE_SIZE: usize = 1900; // for code block safety
const TIMEOUT_SECS: u64 = 5;
const COOLDOWN_SECS: u64 = 10;

static LAST_CALL: Lazy<Mutex<StdHashMap<u64, std::time::Instant>>> =
    Lazy::new(|| Mutex::new(StdHashMap::new()));

pub const NAME: &str = "post";
pub const DESCRIPTION: &str = "HTTP POST を実行します";

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

fn parse_payload_json(s: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(s).map_err(|e| format!("ペイロード JSON の解析に失敗: {e}"))
}

async fn http_post(
    url: &str,
    headers: HashMap<String, String>,
    payload: serde_json::Value,
) -> Result<(Vec<u8>, String), String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("HTTP クライアント作成に失敗: {e}"))?;

    let mut req = client.post(url).json(&payload);
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

fn is_html(content_type: &str) -> bool {
    content_type.starts_with("text/html")
}

fn to_display_text(bytes: &[u8], content_type: &str) -> Option<String> {
    if is_html(content_type) {
        return None;
    }
    if content_type.starts_with("text/") || content_type.starts_with("application/json") {
        String::from_utf8(bytes.to_vec()).ok()
    } else {
        None
    }
}

// プレフィックス: !post <url> <json_payload> [--headers <json>]
pub async fn run(ctx: &Context, msg: &Message) -> serenity::Result<()> {
    // cooldown: decide & drop lock before await
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
            .say(
                &ctx.http,
                "使い方: !post <url> <json_payload> [--headers <json>]",
            )
            .await?;
        return Ok(());
    }

    // parse first token url, second token payload json (may contain spaces -> We try to detect --headers flag)
    // strategy: split by --headers once
    let (left, headers_part) = match rest.split_once(" --headers ") {
        Some(v) => v,
        None => (rest, ""),
    };
    let mut left_iter = left.split_whitespace();
    let url = left_iter.next().unwrap_or("");
    let payload_str = left_iter.collect::<Vec<_>>().join(" "); // join back the remainder as JSON

    if url.is_empty() || payload_str.is_empty() {
        msg.channel_id
            .say(
                &ctx.http,
                "使い方: !post <url> <json_payload> [--headers <json>]",
            )
            .await?;
        return Ok(());
    }

    if let Err(e) = validate_url(url) {
        msg.channel_id.say(&ctx.http, e).await?;
        return Ok(());
    }

    let payload = match parse_payload_json(&payload_str) {
        Ok(p) => p,
        Err(e) => {
            msg.channel_id.say(&ctx.http, e).await?;
            return Ok(());
        }
    };

    let headers = if headers_part.is_empty() {
        HashMap::new()
    } else {
        // Parse headers if provided
        match parse_headers_json(headers_part) {
            Ok(h) => h,
            Err(e) => {
                msg.channel_id.say(&ctx.http, e).await?;
                return Ok(());
            }
        }
    };

    msg.channel_id.say(&ctx.http, "送信中…").await?;

    match http_post(url, headers, payload).await {
        // Handle the HTTP POST response
        Ok((bytes, ct)) => {
            if let Some(mut s) = to_display_text(&bytes, &ct) {
                if s.len() > MAX_MESSAGE_SIZE {
                    // attach as file if not exceeding size cap
                    if bytes.len() > MAX_FILE_SIZE {
                        msg.channel_id
                            .say(
                                &ctx.http,
                                "レスポンスが 10MB を超えたため添付できませんでした",
                            )
                            .await?;
                    } else {
                        let filename = if is_html(&ct) {
                            "response.html"
                        } else {
                            "response.txt"
                        };
                        let attachment = CreateAttachment::bytes(bytes, filename);
                        msg.channel_id
                            .send_message(
                                &ctx.http,
                                CreateMessage::new()
                                    .content("結果が長いためファイルで送信します")
                                    .add_file(attachment),
                            )
                            .await?;
                    }
                } else if ct.starts_with("application/json") {
                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        s = serde_json::to_string_pretty(&json).unwrap_or(s);
                    }
                    let block = format!("```json\n{}\n```", s);
                    msg.channel_id.say(&ctx.http, block).await?;
                } else {
                    let block = format!("```\n{}\n```", s);
                    msg.channel_id.say(&ctx.http, block).await?;
                }
            } else {
                // Non-text or HTML: send as file if under size cap
                if bytes.len() > MAX_FILE_SIZE {
                    msg.channel_id
                        .say(
                            &ctx.http,
                            "レスポンスが 10MB を超えたため添付できませんでした",
                        )
                        .await?;
                } else {
                    let filename = if is_html(&ct) {
                        "response.html"
                    } else {
                        "response.txt"
                    };
                    let attachment = CreateAttachment::bytes(bytes, filename);
                    msg.channel_id
                        .send_message(
                            &ctx.http,
                            CreateMessage::new()
                                .content("結果をファイルで送信します")
                                .add_file(attachment),
                        )
                        .await?;
                }
            }
            Ok(())
        }
        Err(e) => {
            msg.channel_id
                .say(&ctx.http, format!("エラー: {}", e))
                .await?;
            Ok(())
        }
    }
}

// スラッシュ: /post url:<url> payload:<json> headers:<json?>
pub async fn slash_execute(
    ctx: &Context,
    command: &serenity::model::application::CommandInteraction,
) -> serenity::Result<()> {
    // cooldown per user: do not await while locking
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
    let mut payload_s: Option<String> = None;
    let mut headers_json: Option<String> = None;

    for opt in &command.data.options {
        match (opt.name.as_str(), &opt.value) {
            ("url", CommandDataOptionValue::String(s)) => url = Some(s.clone()),
            ("payload", CommandDataOptionValue::String(s)) => payload_s = Some(s.clone()),
            ("headers", CommandDataOptionValue::String(s)) => headers_json = Some(s.clone()),
            _ => {}
        }
    }

    let Some(url) = url else {
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
    };

    let Some(payload_s) = payload_s else {
        command
            .create_response(
                &ctx.http,
                serenity::builder::CreateInteractionResponse::Message(
                    serenity::builder::CreateInteractionResponseMessage::new()
                        .content("payload(JSON) が必要です"),
                ),
            )
            .await?;
        return Ok(());
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

    let payload = match parse_payload_json(&payload_s) {
        Ok(p) => p,
        Err(e) => {
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
    };

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
                // Handle header parsing error
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

    match http_post(&url, headers, payload).await {
        // Handle the HTTP POST response for slash command
        Ok((bytes, ct)) => {
            if let Some(mut s) = super::get::to_display_text(&bytes, &ct) {
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
                        let filename = if super::get::is_html(&ct) {
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
                let filename = if super::get::is_html(&ct) {
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
            CreateCommandOption::new(CommandOptionType::String, "url", "送信先URL").required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "payload",
                "JSON 形式のペイロード",
            )
            .required(true),
        )
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "headers",
            "JSON 形式のヘッダー (任意)",
        ))
}
