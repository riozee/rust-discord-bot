use serenity::{
    builder::CreateCommand,
    model::{application::ResolvedOption, channel::Message},
    prelude::Context,
};

pub async fn run(ctx: &Context, msg: &Message) -> serenity::Result<()> {
    let help_text = "利用可能なコマンド:\n\
- !ping: ポン！と返します\n\
- !help: このヘルプを表示します\n\
- !tex <式>: LaTeX を画像で返します\n\
- !gpt <質問>: tgpt で回答を取得します\n\
- !get <url> [--headers {JSON}]: 指定URLへ GET\n\
- !post <url> <JSON> [--headers {JSON}]: 指定URLへ POST";

    msg.channel_id.say(&ctx.http, help_text).await?;
    Ok(())
}

pub fn slash_run(_options: &[ResolvedOption]) -> String {
    // スラッシュ版も同じ内容を返す
    "利用可能なコマンド:\n- /ping: ポン！と返します\n- /help: このヘルプを表示します\n- /tex formula:<式>: LaTeX を画像で返します\n- /gpt query:<質問>: tgpt で回答を取得します\n- /get url:<url> headers:<JSON?>: HTTP GET\n- /post url:<url> payload:<JSON> headers:<JSON?>: HTTP POST".to_string()
}

// スラッシュコマンド情報
pub const NAME: &str = "help";
pub const DESCRIPTION: &str = "このヘルプを表示します";

pub fn slash_register() -> CreateCommand {
    CreateCommand::new(NAME).description(DESCRIPTION)
}
