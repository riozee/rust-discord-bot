use serenity::{
    builder::{CreateCommand, CreateCommandOption},
    model::{
        application::{CommandOptionType, ResolvedOption, ResolvedValue},
        channel::Message,
    },
    prelude::Context,
};

// スラッシュコマンド情報
pub const NAME: &str = "tex";
pub const DESCRIPTION: &str = "LaTeX をレンダリングして画像URLを返します";

fn build_image_url(latex: &str) -> String {
    // codecogs の PNG 出力を利用。Discord はURLを貼ると展開プレビューされます。
    // 仕様に合わせ、プレアンブル(背景白+解像度)と式を結合して全体を URL エンコード
    let full = format!("\\bg_white\\dpi{{150}} {}", latex);
    let encoded = urlencoding::encode(&full);
    format!("https://latex.codecogs.com/png.latex?{}", encoded)
}

// プレフィックスコマンド: !tex <式>
pub async fn run(ctx: &Context, msg: &Message) -> serenity::Result<()> {
    let content = msg.content.trim();

    // 期待フォーマット: "!tex <latex>"
    let latex = content
        .strip_prefix(super::PREFIX)
        .map(str::trim_start)
        .and_then(|s| s.strip_prefix(NAME))
        .map(str::trim_start)
        .unwrap_or("");

    if latex.is_empty() {
        let usage = r"使い方: !tex <LaTeX 式>
例: !tex \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}";
        msg.channel_id.say(&ctx.http, usage).await?;
        return Ok(());
    }

    let url = build_image_url(latex);
    msg.channel_id.say(&ctx.http, url).await?;
    Ok(())
}

// スラッシュコマンドの実行: /tex formula:<式>
pub fn slash_run(options: &[ResolvedOption]) -> String {
    let formula: Option<String> = options.iter().find_map(|opt| {
        if opt.name == "formula" {
            match &opt.value {
                ResolvedValue::String(s) => Some(s.to_string()),
                _ => None,
            }
        } else {
            None
        }
    });

    match formula {
        Some(s) if !s.trim().is_empty() => build_image_url(s.as_str()),
        _ => "式が指定されていません。/tex で formula: <LaTeX 式> を入力してください。".to_string(),
    }
}

// スラッシュコマンドのメタデータ登録
pub fn slash_register() -> CreateCommand {
    CreateCommand::new(NAME)
        .description(DESCRIPTION)
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "formula",
                "レンダリングしたい LaTeX 式",
            )
            .required(true),
        )
}
