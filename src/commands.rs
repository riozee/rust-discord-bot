// コマンド用モジュール: 各コマンドのハンドラと共通項目を公開

pub mod get;
pub mod help;
pub mod ping;
pub mod post;
pub mod tex;

// プレフィックスはここで設定（後で環境変数などで変更可能）
pub const PREFIX: &str = "!";

use serenity::builder::CreateCommand;

// スラッシュコマンド定義を集約（起動時に自動登録するため）
pub fn slash_commands() -> Vec<CreateCommand> {
    vec![
        ping::slash_register(),
        help::slash_register(),
        tex::slash_register(),
        get::slash_register(),
        post::slash_register(),
    ]
}
