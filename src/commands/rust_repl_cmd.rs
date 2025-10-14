use std::{path::PathBuf, thread::current};

use serenity::{
    builder::{CreateCommand, CreateCommandOption},
    model::{
        application::{CommandOptionType, ResolvedOption, ResolvedValue},
        channel::Message,
    },
    prelude::Context,
};

use crate::rust_repl::rust_repl::{self, CodeRunner};

pub const NAME: &str = "rrepl";
pub const DESCRIPTION: &str = "簡易的なRust REPL";

fn get_location() -> Result<PathBuf, std::io::Error> {
    let current = std::env::current_dir()?;
    let run_loca = current.join("test");
    Ok(run_loca)
}

pub async fn run(ctx: &Context, msg: &Message) -> serenity::Result<()> {
    let content = msg.content.trim();

    // let code_block_remover = |x: &str| {
    //     x.strip_prefix("```")
    //         .and_then(|s| s.strip_suffix("```"))
    //         .unwrap_or(x)
    // };

    let code = content
        .strip_prefix(super::PREFIX)
        .map(str::trim_start)
        .and_then(|s| s.strip_prefix(NAME))
        .map(str::trim_start)
        // remove "```"
        .map(|s| {
            s.strip_prefix("```")
                .and_then(|s| s.strip_suffix("```"))
                .unwrap_or(s)
        })
        .unwrap_or("");

    let res = rust_repl::SrcCode::new(code.trim(), get_location().unwrap(), "2024");

    let out = match res.run() {
        Ok(v) => v,
        Err(e) => format!("{e}"),
    };
    msg.channel_id.say(&ctx.http, out).await?;
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
