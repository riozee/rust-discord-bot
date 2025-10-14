use std::{
    fmt::Display,
    io::Write,
    path::{Path, PathBuf},
    string::FromUtf8Error,
};

const DEFAULT_CARGO_TOML: &str = r#"[package]
name = "tmp"
version = "0.1.0"
edition = ""#;

const TOML_END: &str = r#""
[dependencies]"#;


fn gen_cargo_toml<T: AsRef<str>>(ed: T) -> String {
    format!("{DEFAULT_CARGO_TOML}{}{TOML_END}", ed.as_ref())
}

#[test]
fn test_gen() {
    let ed = "2024";
    println!("{}", gen_cargo_toml(ed));
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    NotInstalledCargo,
    FailedRun(String),
    StrConvert(FromUtf8Error),
    PathIsNotDir,
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(value: FromUtf8Error) -> Self {
        Self::StrConvert(value)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(error) => write!(f, "Io Error: {error}"),
            Error::NotInstalledCargo => write!(f, "not installed Rust run env"),
            Error::FailedRun(e) => write!(f, "failed run {e}"),
            Error::StrConvert(_) => write!(f, "failed convert to String"),
            Error::PathIsNotDir => write!(f, "server error: Path isn't Dir"),
        }
    }
}

// location -> r.g. `~/server/temporary`
#[derive(Debug)]
pub struct SrcCode {
    code: String,
    location: PathBuf,
    edition: String,
}

impl SrcCode {
    pub fn new<T: AsRef<str>, P: AsRef<Path>>(code: T, location: P, edition: T) -> Self {
        Self {
            code: code.as_ref().to_string(),
            location: location.as_ref().to_path_buf(),
            edition: edition.as_ref().to_string(),
        }
    }
}

pub trait CodeRunner {
    fn run(&self) -> Result<String, Error>;
}

impl CodeRunner for SrcCode {
    fn run(&self) -> Result<String, Error> {
        let pj_mainrs_pathes = ready_work_env(&self.location, self.edition.clone())?;
        let mut main_rs = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(pj_mainrs_pathes.1)?;
        //                                   self is reference -> use clone()
        let formatted_code = ready_code(self.code.clone());
        main_rs.write_all(formatted_code.as_bytes())?;

        let result = std::process::Command::new("cargo")
            .args(["run"])
            .current_dir(pj_mainrs_pathes.0)
            .output()?;
        let result_str = String::from_utf8(result.stdout)?;
        let result_err = String::from_utf8(result.stderr)?;

        if !result.status.success() {
            Err(Error::FailedRun(result_err))
        } else {
            Ok(result_str)
        }
    }
}

// ready Rust temporary project folder
// return (project_path, main.rs)
fn ready_work_env<T: AsRef<str>, P: AsRef<Path>>(path: P, edition: T) -> Result<(PathBuf, PathBuf), Error> {
    if runnable_rust() {
        // init_project_dir(&path)?;
        if path.as_ref().exists() {
            std::fs::remove_dir_all(&path)?;
        }
        std::fs::create_dir_all(&path)?;

        let src_dir_path = path.as_ref().join("src");
        std::fs::create_dir_all(&src_dir_path)?;
        let main_rs_path = src_dir_path.join("main.rs");

        // when main.rs exist, overwrite as 0.
        std::fs::File::create(&main_rs_path)?;

        let cargo_toml_path = path.as_ref().join("Cargo.toml");
        let mut res = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&cargo_toml_path)?;
        res.write_all(gen_cargo_toml(edition).as_bytes())?;

        Ok((path.as_ref().to_path_buf(), main_rs_path))
    } else {
        Err(Error::NotInstalledCargo)
    }
}

fn init_project_dir<P: AsRef<Path>>(path: P) -> Result<(), Error> {
    if !path.as_ref().is_dir() {
        std::fs::remove_dir_all(&path)?;
    }
    Ok(std::fs::create_dir_all(&path)?)
}

fn ready_code<T: AsRef<str>>(src: T) -> String {
    format!("fn main(){{{}}}", src.as_ref())
}

fn runnable_rust() -> bool {
    match std::process::Command::new("cargo")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(v) => v.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::rust_repl::rust_repl::{CodeRunner, SrcCode};

    #[test]
    fn test_run() {
        let c_path = std::env::current_dir().unwrap();
        let f = c_path.join("test");
        assert!(f.exists());
        let src = SrcCode::new(r#"println!("helloworld!!!");"#, c_path.join("test"), "2024");
        let result = src.run();
        println!("{result:?}");
    }
}
