use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use crate::Options;

#[derive(Parser)]
#[command(name = "symapper", version, about)]
struct Args {
    repo: PathBuf,
    #[arg(long, value_name = "path|ident")]
    focus: Vec<String>,
    #[arg(long, default_value_t = 1500, value_name = "N", value_parser = positive_tokens)]
    tokens: usize,
    #[arg(long, value_name = "glob")]
    exclude: Vec<String>,
    #[arg(long)]
    no_cache: bool,
}

fn positive_tokens(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(tokens) if tokens > 0 => Ok(tokens),
        _ => Err("tokens must be a positive integer".to_owned()),
    }
}

pub fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let _ = error.print();
            return ExitCode::from(error.exit_code() as u8);
        }
    };
    let opts = Options {
        repo: args.repo,
        focus: args.focus,
        tokens: args.tokens,
        exclude: args.exclude,
        no_cache: args.no_cache,
        cache_dir: std::env::var_os("SYMAPPER_CACHE_DIR"),
    };
    match crate::map(&opts) {
        Ok(output) => {
            print!("{}", output.text);
            if let Some(note) = output.note {
                eprintln!("{note}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("symapper: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
