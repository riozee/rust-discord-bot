use reqwest;
use serde::{Deserialize, Serialize};
use serenity::{
    all::CommandDataOptionValue,
    builder::{CreateCommand, CreateCommandOption},
    model::application::CommandOptionType,
    prelude::Context,
};
use std::{collections::HashMap, fmt::Display};

pub const NAME: &str = "eval";
pub const DESCRIPTION: &str = "REPL";

pub fn slash_register() -> CreateCommand {
    CreateCommand::new(NAME)
        .description(DESCRIPTION)
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "code", DESCRIPTION).required(true),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "lang", "language")
                .required(true)
                .add_string_choice("Rust", "rust")
                .add_string_choice("Python", "python")
                .add_string_choice("C", "c")
                .add_string_choice("C++", "cpp")
                .add_string_choice("Jave", "java")
                .add_string_choice("JavaScript", "javascript")
                .add_string_choice("TypeScript", "typescript")
                .add_string_choice("Go", "go")
                .add_string_choice("Ruby", "ruby")
                .add_string_choice("bash", "bash")
                .add_string_choice("haskell", "haskell")
                .add_string_choice("lisp", "lisp")
                .add_string_choice("ocaml", "ocaml")
                .add_string_choice("prolog", "prolog")
                .add_string_choice("zig", "zig")
                .add_string_choice("swift", "swift")
                .add_string_choice("scala", "scala")
                .add_string_choice("nim", "nim"),
        )
}

pub async fn slash_execute(
    ctx: &Context,
    command: &serenity::model::application::CommandInteraction,
) -> serenity::Result<()> {
    command.defer(&ctx.http).await?;
    // required(true)のためunwrap
    let code_opt = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "code")
        .unwrap();
    let lang_opt = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "lang")
        .unwrap();

    let code = if let CommandDataOptionValue::String(code_val) = code_opt.value.clone() {
        code_val
    } else {
        command
            .edit_response(
                &ctx,
                serenity::builder::EditInteractionResponse::new().content("コードが不正です。"),
            )
            .await?;
        return Ok(());
    };
    let lang = if let CommandDataOptionValue::String(lang_val) = lang_opt.value.clone() {
        lang_val
    } else {
        command
            .edit_response(
                &ctx,
                serenity::builder::EditInteractionResponse::new().content("言語が不正です。"),
            )
            .await?;
        return Ok(());
    };

    let langs = match Languages::get_from_api().await {
        Ok(l) => l,
        Err(_) => {
            command
                .edit_response(
                    &ctx,
                    serenity::builder::EditInteractionResponse::new()
                        .content("言語リストの取得に失敗しました。"),
                )
                .await?;
            return Ok(());
        }
    };
    let lang = match langs.get(&lang) {
        Some(l) => l,
        None => {
            command
                .edit_response(
                    &ctx,
                    serenity::builder::EditInteractionResponse::new().content("非対応の言語です。"),
                )
                .await?;
            return Ok(());
        }
    };

    let req_info = ReqJson::new(lang, code.clone());
    println!("{req_info:?}");
    let res = match run_with_api(req_info).await {
        Ok(r) => r,
        Err(_) => {
            command
                .edit_response(
                    &ctx,
                    serenity::builder::EditInteractionResponse::new()
                        .content("実行に失敗しました。"),
                )
                .await?;
            return Ok(());
        }
    };

    command
        .edit_response(
            &ctx,
            serenity::builder::EditInteractionResponse::new()
                .content(format!("```{}\n{code}\n```\n{res}", lang.language)),
        )
        .await?;

    // command.edit_response(cache_http, builder)

    Ok(())
}

async fn run_with_api(req_info: ReqJson) -> Result<String, reqwest::Error> {
    let client = reqwest::Client::new();
    let res = client
        .post("https://emkc.org/api/v2/piston/execute")
        .json(&req_info)
        .send()
        .await?;
    let res = res.json::<Resp>().await?;
    Ok(format!("{res}"))
}

/// レスポンスのデシアライズ用のstruct
/// 多言語対応のため必要なフィールドだけ実装
#[derive(Debug, Serialize, Deserialize)]
struct Resp {
    language: String,
    version: String,
    run: Run,
}

/// 結果表示用。このままmsgに流してる
impl Display for Resp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "lang: {}\nversion: {}\nresult:\n```bash\n{}```",
            self.language, self.version, self.run.output
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Run {
    stdout: String,
    stderr: String,
    code: i32,
    output: String,
}

/// apiリクエスト用
#[derive(Debug, Serialize, Deserialize)]
struct ReqJson {
    language: String,
    version: String,
    files: Vec<FileContent>,
}

impl ReqJson {
    /// lang引数はLang struct。Lang structはversion情報を含む
    fn new(lang: &Lang, code: String) -> Self {
        Self {
            language: lang.language.clone(),
            version: lang.version.clone(),
            files: vec![FileContent::new(lang, code)],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FileContent {
    name: String,
    content: String,
}

impl FileContent {
    fn new<T: AsRef<str>>(lang: &Lang, code: T) -> Self {
        let code = code.as_ref().to_string();
        Self {
            name: format!("main.{}", lang_to_extension(lang)),
            content: code_generator(code, lang),
        }
    }
}

/// 言語名から拡張子を生成
fn lang_to_extension(lang: &Lang) -> String {
    let lang = lang.language.clone();
    // 言語 → 拡張子 のテーブル
    let table: HashMap<&str, &str> = [
        ("rust", "rs"),
        ("python", "py"),
        ("c++", "cpp"),
        ("c", "c"),
        ("java", "java"),
        ("javascript", "js"),
        ("typescript", "ts"),
        ("go", "go"),
        ("ruby", "rb"),
        ("html", "html"),
        ("css", "css"),
        ("bash", "bash"),
        ("haskell", "hs"),
        ("lisp", "lisp"),
        ("ocaml", "ml"),
        ("prolog", "pl"),
        ("zig", "zig"),
        ("swift", "swift"),
        ("scala", "sc"),
        ("nim", "nim"),
    ]
    .iter()
    .cloned()
    .collect();

    table
        .get(lang.to_lowercase().as_str())
        .unwrap_or(&"rs")
        .to_string()
}

/// main() {}が必要かどうか
fn reqire_main(lang: &Lang) -> bool {
    matches!(
        lang.language.to_lowercase().as_str(),
        "rust" | "c++" | "c" | "go" | "java" | "zig"
    )
}

/// `reqire_main()`によってmain(){}などを追加して実行可能なコードを生成
fn code_generator<T: AsRef<str>>(code: T, lang: &Lang) -> String {
    let code = code.as_ref().to_string();
    if reqire_main(lang) {
        let lang_name = lang.language.clone();
        match lang_name.as_str() {
            "rust" => format!("fn main() {{{code}}}"),
            "c" | "c++" => format!("int main() {{{code}}}"),
            "go" => format!("func main() {{{code}}}"),
            "java" => {
                format!("public class Main {{public static void main(String[] args) {{{code}}}}}")
            }
            // for zig
            _ => format!("pub fn main() void {{{code}}}"),
        }
    } else {
        println!("{code}");
        code
    }
}

/// api対応言語を取得。
/// TODO: キャッシュ用に24h程度の期限で外部保存できるようにする
#[derive(Debug, Serialize, Deserialize)]
struct Languages(Vec<Lang>);

impl Languages {
    pub async fn get_from_api() -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::new();
        let res = client
            .get("https://emkc.org/api/v2/piston/runtimes")
            .send()
            .await?;
        Ok(Self(res.json::<Vec<Lang>>().await?))
    }

    fn get<T: AsRef<str>>(&self, lang: T) -> Option<&Lang> {
        self.0.iter().find(|s| s.language == lang.as_ref())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Lang {
    language: String,
    version: String,
}

// struct Cache {
//     time_stamp: chrono::DateTime<chrono::Utc>,
//     content: Languages,
// }

// impl Cache {
//     async fn new() -> Result<Self, reqwest::Error> {
//         Ok(Self {
//             time_stamp: chrono::Utc::now(),
//             content: Languages::get_from_api().await?,
//         })
//     }
//     fn is_cache_outdated(&self) -> bool {
//         let now = chrono::Utc::now();
//         let elapsed = now - self.time_stamp;
//         elapsed > chrono::Duration::hours(24)
//     }
//     fn get_about_lang<T: AsRef<str>>(&self, lang: T) -> Option<&Lang> {
//         // 本当はHashMapにした方が検索時間が減るが、apiはリストを返すので変換が不便
//         // また要素数は極めて限定的なため検索時間におけるオーバーヘッドは無視できるはす
//         self.content
//             .0
//             .iter()
//             .find(|s| s.language == lang.as_ref().to_string())
//     }
//     async fn update(&mut self) -> Result<(), reqwest::Error> {
//         let lst = Languages::get_from_api().await?;
//         self.time_stamp = chrono::Utc::now();
//         self.content = lst;
//         Ok(())
//     }
//     fn check(&mut self) {
//         if self.is_cache_outdated() {
//             self.update();
//         }
//     }
// }
