use serenity::all::{
    CommandDataOptionValue, CommandOptionType, Context, CreateCommand, CreateCommandOption,
};
use unicode_width::UnicodeWidthChar;

pub const NAME: &str = "huki";
pub const DESCRIPTION: &str = "totuzen no shi generator";

pub fn slash_register() -> CreateCommand {
    CreateCommand::new(NAME)
        .description(DESCRIPTION)
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "content", "totuzen no shi")
                .required(true),
        )
}

fn get_str_len<T: AsRef<str>>(strg: T) -> u32 {
    let mut len = 0;
    for c in strg.as_ref().chars() {
        len += c.width().unwrap_or(0);
    }
    len as u32
}

fn get_max_len<T: AsRef<str>>(strg: T) -> u32 {
    let mut max = 0;
    for l in strg.as_ref().lines() {
        let len = get_str_len(l);
        if max < len {
            max = len;
        }
    }
    max
}

fn mul_str<T: AsRef<str> + Sized>(msg: &T, mul: u32) -> String {
    let mut res = String::new();
    for _ in 0..mul {
        res.push_str(msg.as_ref());
    }
    res
}

// 突然の死!!!
//
// 👇にする
//
// ＿人人人人人人人＿
// ＞ 突然の死!!! ＜
// ￣人人人人人人人￣
fn s2huki<T: AsRef<str>>(s: T) -> String {
    let max_width = get_max_len(&s);
    let top = format!("＿{}＿\n", mul_str(&"人", max_width / 2));
    let btm = format!("￣{}￣\n", mul_str(&"Y^", max_width / 2));
    let mut ss = String::new();
    ss.push_str(&top);
    for l in s.as_ref().lines() {
        let fit_spc = mul_str(&" ", max_width - get_str_len(l));
        ss.push_str(&format!("＞ {}{} ＜\n", l, fit_spc));
    }
    ss.push_str(&btm);
    ss
}

pub async fn slash_execute(
    ctx: &Context,
    command: &serenity::model::application::CommandInteraction,
) -> serenity::Result<()> {
    let input = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "content")
        .unwrap()
        .value
        .clone();

    let c = if let CommandDataOptionValue::String(cc) = input {
        s2huki(cc)
    } else {
        "なんかダメだったぁ".to_string()
    };
    command
        .create_response(
            &ctx.http,
            serenity::builder::CreateInteractionResponse::Message(
                serenity::builder::CreateInteractionResponseMessage::new().content(c),
            ),
        )
        .await?;
    Ok(())
}

#[test]
fn test_s2huki() {
    let foo = "突然の死";
    let bar = "foo\nbar\nfoobar";

    println!("{}", s2huki(foo));
    println!("{}", s2huki(bar));
}
