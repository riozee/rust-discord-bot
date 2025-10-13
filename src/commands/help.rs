use serenity::{model::channel::Message, prelude::Context};

pub async fn run(ctx: &Context, msg: &Message) -> serenity::Result<()> {
    let help_text = "利用可能なコマンド:\n\
- !ping: ポン！と返します\n\
- !help: このヘルプを表示します";

    msg.channel_id.say(&ctx.http, help_text).await?;
    Ok(())
}
