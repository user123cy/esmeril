use std::path::Path;

use colored::Colorize;
use serde::Serialize;

use crate::cli::BuildArgs;

#[derive(Serialize)]
pub struct BuildReport {
    pub path: String,
    pub grade: char,
    pub score: u8,
    pub output: String,
    pub ok: bool,
    pub summary: String,
}

impl BuildReport {
    pub fn fail(&self) -> bool {
        !self.ok
    }
}

pub fn run(args: &BuildArgs, json: bool) -> anyhow::Result<BuildReport> {
    let root = Path::new(&args.path);
    let check = crate::check::inspect(root);
    if check.fail() {
        crate::check::print_report(&check);
        anyhow::bail!("check failed - fix the project before building");
    }
    if !crate::check::on_path("rojo") {
        anyhow::bail!("rojo is not installed - install with: aftman add rojo-rbx/rojo");
    }

    let default_out = if crate::fix::is_lib(root) {
        "lib.rbxm"
    } else {
        "game.rbxl"
    };
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| default_out.to_string());
    let out = crate::tool::run("rojo", &["build", "-o", &output], root)?;
    let report = BuildReport {
        path: root.display().to_string(),
        grade: check.grade,
        score: check.score,
        output,
        ok: out.code == 0,
        summary: crate::tool::summary(&out),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(report)
}

fn print_report(r: &BuildReport) {
    println!("  {}  {}", "esmeril build".bold().cyan(), r.path);
    println!();
    println!("  check  {}", crate::check::colored_grade(r.grade, r.score));
    let mark = if r.ok { "✓".green() } else { "✗".red() };
    let summary = if r.summary.is_empty() {
        if r.ok {
            "ok".to_string()
        } else {
            "build failed".to_string()
        }
    } else {
        r.summary.clone()
    };
    let line = format!("  {} rojo  {}", mark, summary);
    if r.ok {
        println!("{}  ({})", line, r.output.dimmed());
    } else {
        println!("{}", line.red());
    }
    println!();
    if r.ok {
        println!("  {}", format!("wrote {}", r.output).green());
    } else {
        println!("  {}", "build failed".red().bold());
    }
}
