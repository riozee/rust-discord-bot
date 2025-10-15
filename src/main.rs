use dotenv::dotenv;
use std::env;

use serenity::async_trait;
use serenity::model::{
    application::{Command, Interaction},
    channel::Message,
    gateway::Ready,
    id::GuildId,
};
use serenity::prelude::*;

mod commands;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, _ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let name = command.data.name.as_str();
            match name {
                commands::ping::NAME => {
                    let content = commands::ping::slash_run(&command.data.options());
                    if let Err(why) = command
                        .create_response(
                            &_ctx.http,
                            serenity::builder::CreateInteractionResponse::Message(
                                serenity::builder::CreateInteractionResponseMessage::new()
                                    .content(content),
                            ),
                        )
                        .await
                    {
                        println!("スラッシュコマンドの応答に失敗: {why:?}");
                    }
                }
                commands::help::NAME => {
                    let content = commands::help::slash_run(&command.data.options());
                    if let Err(why) = command
                        .create_response(
                            &_ctx.http,
                            serenity::builder::CreateInteractionResponse::Message(
                                serenity::builder::CreateInteractionResponseMessage::new()
                                    .content(content),
                            ),
                        )
                        .await
                    {
                        println!("スラッシュコマンドの応答に失敗: {why:?}");
                    }
                }
                commands::tex::NAME => {
                    let content = commands::tex::slash_run(&command.data.options());
                    if let Err(why) = command
                        .create_response(
                            &_ctx.http,
                            serenity::builder::CreateInteractionResponse::Message(
                                serenity::builder::CreateInteractionResponseMessage::new()
                                    .content(content),
                            ),
                        )
                        .await
                    {
                        println!("スラッシュコマンドの応答に失敗: {why:?}");
                    }
                }
                commands::get::NAME => {
                    if let Err(why) = commands::get::slash_execute(&_ctx, &command).await {
                        println!("/get 実行エラー: {why:?}");
                    }
                }
                commands::post::NAME => {
                    if let Err(why) = commands::post::slash_execute(&_ctx, &command).await {
                        println!("/post 実行エラー: {why:?}");
                    }
                }
                _ => {
                    if let Err(why) = command
                        .create_response(
                            &_ctx.http,
                            serenity::builder::CreateInteractionResponse::Message(
                                serenity::builder::CreateInteractionResponseMessage::new()
                                    .content("未対応のコマンドです"),
                            ),
                        )
                        .await
                    {
                        println!("スラッシュコマンドの応答に失敗: {why:?}");
                    }
                }
            }
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        // ループ防止のため、Bot自身および他のBotのメッセージは無視
        if msg.author.bot {
            return;
        }

        // シンプルなプレフィックス解析
        let content = msg.content.trim();
        if !content.starts_with(commands::PREFIX) {
            return;
        }

        // プレフィックスを外し、コマンドと引数に分割
        let without_prefix = content[commands::PREFIX.len()..].trim();
        if without_prefix.is_empty() {
            return;
        }
        let mut parts = without_prefix.split_whitespace();
        let command = parts.next().unwrap_or("");
        // let args: Vec<&str> = parts.collect(); // 将来のために引数を使う場合

        // コマンドごとのハンドラにディスパッチ
        let result = match command {
            "ping" => commands::ping::run(&ctx, &msg).await,
            "help" => commands::help::run(&ctx, &msg).await,
            "tex" => commands::tex::run(&ctx, &msg).await,
            "get" => commands::get::run(&ctx, &msg).await,
            "post" => commands::post::run(&ctx, &msg).await,
            _ => Ok(()), // 不明なコマンドは現状スルー
        };

        if let Err(why) = result {
            println!("コマンド '{}' の実行中にエラーが発生: {:?}", command, why);
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} として接続しました", ready.user.name);
        // グローバルコマンドとして登録（反映に最大1時間）
        let cmds = commands::slash_commands();
        match Command::set_global_commands(&ctx.http, cmds).await {
            Ok(commands) => println!("登録されたグローバルスラッシュコマンド: {commands:#?}"),
            Err(why) => println!("スラッシュコマンド登録に失敗: {why:?}"),
        }

        // 開発用: GUILD_ID が設定されていればギルドコマンドとして即時反映
        if let Ok(guild_id_str) = std::env::var("GUILD_ID")
            && let Ok(id) = guild_id_str.parse::<u64>()
        {
            let guild_id = GuildId::new(id);
            match guild_id
                .set_commands(&ctx.http, commands::slash_commands())
                .await
            {
                Ok(commands) => println!("ギルド({id})のスラッシュコマンド: {commands:#?}"),
                Err(why) => println!("ギルドへのスラッシュコマンド登録に失敗: {why:?}"),
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok(); // .env をロード

    let token = env::var("DISCORD_TOKEN").expect("環境変数にトークンが必要です (DISCORD_TOKEN)");

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("クライアントの作成に失敗しました");

    if let Err(why) = client.start().await {
        println!("クライアントエラー: {:?}", why);
    }
}
