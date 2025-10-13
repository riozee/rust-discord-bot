use serenity::{model::channel::Message, prelude::Context};

pub async fn run(ctx: &Context, msg: &Message) -> serenity::Result<()> {
    // シンプルな応答コマンド
    msg.channel_id.say(&ctx.http, "ポン！").await?;
    Ok(())
}
