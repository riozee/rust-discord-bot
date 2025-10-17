use reqwest;
use serde::{Deserialize, Serialize};
use serenity::{
    builder::{CreateCommand, CreateCommandOption},
    model::{application::CommandOptionType, channel::Message},
    prelude::Context,
};
use std::fmt::Display;

// use crate::rust_repl::rust_repl::{self, CodeRunner};

pub const NAME: &str = "rrepl";
pub const DESCRIPTION: &str = "簡易的なRust REPL";

fn code_format<T: AsRef<str>>(str: T) -> (String, String) {
    let str = str
        .as_ref()
        .strip_prefix(super::PREFIX)
        .map(str::trim_start)
        .and_then(|s| s.strip_prefix(NAME))
        .map(str::trim_start)
        .unwrap_or("");

    match str.strip_prefix("```").and_then(|s| s.strip_suffix("```")) {
        Some(v) => {
            let inner = v.trim();
            let mut lines = inner.splitn(2, '\n');
            let lang = lines.next().unwrap_or("").trim().to_string();
            let code = lines.next().unwrap_or("").trim().to_string();
            (lang, code)
        }
        None => ("rust".to_string(), str.to_string()),
    }
}

pub async fn run(ctx: &Context, msg: &Message) -> serenity::Result<()> {
    let content = msg.content.trim();

    let code = code_format(content);

    let res = call_api(code.0, code.1)
        .await
        .map_err(|_| serenity::Error::Other("api error"))?;
    msg.channel_id.say(&ctx.http, res).await?;
    Ok(())
}

pub fn slash_register() -> CreateCommand {
    CreateCommand::new(NAME)
        .description(DESCRIPTION)
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "code", "簡易実行するRustコード")
                .required(true),
        )
}

#[derive(Debug, Serialize, Deserialize)]
struct ReqJson {
    language: String,
    version: String,
    files: Vec<FileContent>,
}

impl ReqJson {
    fn new(lang: String, code: String) -> Self {
        Self {
            language: lang,
            version: "1.68.2".to_string(),
            files: vec![FileContent::new(code)],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FileContent {
    name: String,
    content: String,
}

impl FileContent {
    fn new<T: AsRef<str>>(code: T) -> Self {
        let code = code.as_ref().to_string();
        Self {
            name: "main.rs".to_string(),
            content: format!("fn main() {{{code}}}"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Resp {
    language: String,
    version: String,
    run: Run,
    compile: Compile,
}

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

#[derive(Debug, Serialize, Deserialize)]
struct Compile {
    stdout: String,
}

pub async fn call_api<T: AsRef<str>>(lang: T, code: T) -> Result<String, reqwest::Error> {
    let client = reqwest::Client::new();
    let req_info = ReqJson::new(lang.as_ref().to_string(), code.as_ref().to_string());
    let res = client
        .post("https://emkc.org/api/v2/piston/execute")
        .json(&req_info)
        .send()
        .await?;
    let res = res.json::<Resp>().await?;
    Ok(format!("{res}"))
}

#[cfg(test)]
mod tests {
    use crate::commands::rust_repl_cmd::{FileContent, call_api, code_format};

    #[test]
    fn test_api() {
        let code = r#"println!("Hello worold!");"#;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt.block_on(async {
            let res = call_api("rust", code).await.unwrap();
            res
        });
        // println!("{res}");
        let ans = r#"lang: rust
version: 1.68.2
result:
```bash
Hello worold!
```"#;
        assert_eq!(res, ans);
    }

    #[test]
    fn test_emb_code() {
        let code = r#"println!("Hello worold!");"#;
        let genf = FileContent::new(code);
        assert_eq!(genf.content, "fn main() {println!(\"Hello worold!\");}");
    }

    #[test]
    fn cd_fmt_test() {
        let code = r#"!rrepl
```rust
println!("HEllo World");
```"#;
        let res = code_format(code);
        println!("lang: {}\ncode: {}", res.0, res.1);

        let code = r#"!rrepl
```
println!("HEllo World");
```"#;
        let res = code_format(code);
        println!("lang: {}\ncode: {}", res.0, res.1);

        let code = r#"!rrepl
```python
println!("HEllo World");
```"#;
        let res = code_format(code);
        println!("lang: {}\ncode: {}", res.0, res.1);
    }
}
