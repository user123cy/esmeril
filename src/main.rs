mod build;
mod cache;
mod check;
mod cli;
mod deps;
mod doctor;
mod fix;
mod fmt_cmd;
mod init;
mod template;
mod tool;
mod update;

use clap::{CommandFactory, Parser};

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    match cli.command {
        cli::Commands::Init(args) => init::run(&args)?,
        cli::Commands::Check(args) => {
            let report = check::run(&args, cli.json)?;
            if report.fail() {
                std::process::exit(1);
            }
        }
        cli::Commands::Deps(args) => {
            let report = deps::run(&args, cli.json)?;
            if report.fail() {
                std::process::exit(1);
            }
        }
        cli::Commands::Doctor => {
            let report = doctor::run(cli.json)?;
            if report.fail() {
                std::process::exit(1);
            }
        }
        cli::Commands::Update(args) => {
            let report = update::run(&args, cli.json)?;
            if report.fail() {
                std::process::exit(1);
            }
        }
        cli::Commands::Fmt(args) => {
            let report = fmt_cmd::run(&args, cli.json)?;
            if report.fail() {
                std::process::exit(1);
            }
        }
        cli::Commands::Build(args) => {
            let report = build::run(&args, cli.json)?;
            if report.fail() {
                std::process::exit(1);
            }
        }
        cli::Commands::Completions { shell } => {
            let mut cmd = cli::Cli::command();
            clap_complete::generate(shell, &mut cmd, "esmeril", &mut std::io::stdout());
        }
    }
    Ok(())
}
