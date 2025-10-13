use dotenv::dotenv;
use std::env;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

mod commands;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
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
            _ => Ok(()), // 不明なコマンドは現状スルー
        };

        if let Err(why) = result {
            println!("コマンド '{}' の実行中にエラーが発生: {:?}", command, why);
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
