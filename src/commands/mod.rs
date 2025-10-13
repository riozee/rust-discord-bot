// コマンド用モジュール: 各コマンドのハンドラと共通項目を公開

pub mod help;
pub mod ping;

// プレフィックスはここで設定（後で環境変数などで変更可能）
pub const PREFIX: &str = "!";
