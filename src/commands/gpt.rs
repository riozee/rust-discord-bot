use serenity::{
    builder::{CreateCommand, CreateCommandOption, EditAttachments},
    model::{
        application::{CommandDataOptionValue, CommandOptionType},
        channel::Message,
    },
    prelude::Context,
};

use tokio::process::Command;

const MAX_MESSAGE_SIZE: usize = 1900; // safety margin for code blocks
const MAX_FILE_SIZE: usize = 10_000_000; // 10 MB
const TIMEOUT_SECS: u64 = 30; // external tool timeout

pub const NAME: &str = "gpt";
pub const DESCRIPTION: &str = "tgpt で回答を取得します";

async fn run_tgpt(query: &str, preprompt: &str) -> Result<Vec<u8>, String> {
    // Run: tgpt --quiet --preprompt <preprompt> "query"
    let mut cmd = Command::new("tgpt");
    cmd.arg("--quiet")
        .arg("--preprompt")
        .arg(preprompt)
        .arg(query);

    // Apply timeout to avoid hanging
    match tokio::time::timeout(std::time::Duration::from_secs(TIMEOUT_SECS), cmd.output()).await {
        Err(_) => Err(format!("タイムアウトしました ({}秒)", TIMEOUT_SECS)),
        Ok(Err(e)) => Err(format!("コマンド実行エラー: {}", e)),
        Ok(Ok(output)) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let msg = if stderr.trim().is_empty() {
                    "tgpt 実行に失敗しました (詳細不明)".to_string()
                } else {
                    format!("tgpt 実行に失敗: {}", stderr.trim())
                };
                Err(msg)
            } else {
                Ok(output.stdout)
            }
        }
    }
}

fn to_message_or_file_bytes(bytes: Vec<u8>) -> Result<String, (Vec<u8>, String)> {
    // Try UTF-8; fallback to file if not UTF-8
    match String::from_utf8(bytes.clone()) {
        Ok(mut s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Ok("(空の応答)".to_string());
            }
            // Compact excessive whitespace lines
            s = trimmed.to_string();
            if s.len() <= MAX_MESSAGE_SIZE {
                Ok(s)
            } else {
                Err((bytes, "gpt.txt".to_string()))
            }
        }
        Err(_) => Err((bytes, "gpt.bin".to_string())),
    }
}

// Prefix: !gpt <質問>
pub async fn run(ctx: &Context, msg: &Message) -> serenity::Result<()> {
    let content = msg.content.trim();
    let query = content
        .strip_prefix(super::PREFIX)
        .map(str::trim_start)
        .and_then(|s| s.strip_prefix(NAME))
        .map(str::trim)
        .unwrap_or("");

    if query.is_empty() {
        msg.channel_id.say(&ctx.http, "使い方: !gpt <質問>").await?;
        return Ok(());
    }

    // Build preprompt, appending replied message content if present
    let mut preprompt = String::from(
        "あなたの名前は'rust-bot'。ソフトウエア研究サークルのDiscordボット。*respond in brief*.",
    );
    if let Some(referenced) = &msg.referenced_message {
        let replied = referenced.content.trim();
        if !replied.is_empty() {
            preprompt.push_str("\nThe content of the last message: ");
            preprompt.push_str(replied);
        }
    }

    match run_tgpt(query, &preprompt).await {
        Ok(bytes) => match to_message_or_file_bytes(bytes) {
            Ok(text) => {
                // Respond as plain text (no code block)
                msg.channel_id.say(&ctx.http, text).await?;
                Ok(())
            }
            Err((bytes, filename)) => {
                if bytes.len() > MAX_FILE_SIZE {
                    msg.channel_id
                        .say(
                            &ctx.http,
                            "応答が長すぎるため送信できませんでした (10MB 超) ✋",
                        )
                        .await?;
                    return Ok(());
                }
                let attachment = serenity::builder::CreateAttachment::bytes(bytes, filename);
                msg.channel_id
                    .send_message(
                        &ctx.http,
                        serenity::builder::CreateMessage::new()
                            .content("回答が長いためファイルで送信します")
                            .add_file(attachment),
                    )
                    .await?;
                Ok(())
            }
        },
        Err(e) => {
            msg.channel_id
                .say(
                    &ctx.http,
                    format!(
                        "エラー: {}\n' tgpt ' がインストールされているか確認してください。",
                        e
                    ),
                )
                .await?;
            Ok(())
        }
    }
}

// Slash: /gpt query:<質問>
pub async fn slash_execute(
    ctx: &Context,
    command: &serenity::model::application::CommandInteraction,
) -> serenity::Result<()> {
    let mut query: Option<String> = None;
    for opt in &command.data.options {
        if let ("query", CommandDataOptionValue::String(s)) = (opt.name.as_str(), &opt.value) {
            query = Some(s.clone());
        }
    }

    let Some(query) = query.filter(|q| !q.trim().is_empty()) else {
        command
            .create_response(
                &ctx.http,
                serenity::builder::CreateInteractionResponse::Message(
                    serenity::builder::CreateInteractionResponseMessage::new()
                        .content("query が必要です"),
                ),
            )
            .await?;
        return Ok(());
    };

    // Defer immediately (tgpt may take time)
    command
        .create_response(
            &ctx.http,
            serenity::builder::CreateInteractionResponse::Defer(
                serenity::builder::CreateInteractionResponseMessage::new(),
            ),
        )
        .await?;

    // For slash commands, there is no replied message context; use base preprompt
    match run_tgpt(&query, "respond in brief").await {
        Ok(bytes) => match to_message_or_file_bytes(bytes) {
            Ok(text) => {
                command
                    .edit_response(
                        &ctx.http,
                        serenity::builder::EditInteractionResponse::new().content(text),
                    )
                    .await?;
                Ok(())
            }
            Err((bytes, filename)) => {
                if bytes.len() > MAX_FILE_SIZE {
                    command
                        .edit_response(
                            &ctx.http,
                            serenity::builder::EditInteractionResponse::new()
                                .content("応答が長すぎるため送信できませんでした (10MB 超) ✋"),
                        )
                        .await?;
                    return Ok(());
                }
                let files = EditAttachments::new()
                    .add(serenity::builder::CreateAttachment::bytes(bytes, filename));
                command
                    .edit_response(
                        &ctx.http,
                        serenity::builder::EditInteractionResponse::new()
                            .content("回答が長いためファイルで送信します")
                            .attachments(files),
                    )
                    .await?;
                Ok(())
            }
        },
        Err(e) => {
            command
                .edit_response(
                    &ctx.http,
                    serenity::builder::EditInteractionResponse::new().content(format!(
                        "エラー: {}\n' tgpt ' がインストールされているか確認してください。",
                        e
                    )),
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
            CreateCommandOption::new(CommandOptionType::String, "query", "質問/プロンプト")
                .required(true),
        )
}
