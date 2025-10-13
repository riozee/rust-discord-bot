use serenity::{
    builder::CreateCommand,
    model::{application::ResolvedOption, channel::Message},
    prelude::Context,
};

pub async fn run(ctx: &Context, msg: &Message) -> serenity::Result<()> {
    // シンプルな応答コマンド
    msg.channel_id.say(&ctx.http, "ポン！").await?;
    Ok(())
}

// スラッシュコマンドの実行
pub fn slash_run(_options: &[ResolvedOption]) -> String {
    "ポン！".to_string()
}

// スラッシュコマンド情報
pub const NAME: &str = "ping";
pub const DESCRIPTION: &str = "ポン！と返します";

// スラッシュコマンドのメタデータ登録
pub fn slash_register() -> CreateCommand {
    CreateCommand::new(NAME).description(DESCRIPTION)
}
