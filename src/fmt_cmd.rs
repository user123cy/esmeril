use std::path::Path;

use colored::Colorize;
use serde::Serialize;

use crate::cli::FmtArgs;

#[derive(Serialize)]
pub struct FmtReport {
    pub path: String,
    pub check: bool,
    pub tools: Vec<ToolRow>,
    pub broken: usize,
}

#[derive(Serialize)]
pub struct ToolRow {
    pub name: &'static str,
    pub ok: bool,
    pub summary: String,
    pub hint: Option<&'static str>,
}

impl FmtReport {
    pub fn fail(&self) -> bool {
        self.broken > 0
    }
}

pub fn run(args: &FmtArgs, json: bool) -> anyhow::Result<FmtReport> {
    let root = Path::new(&args.path);
    let src = root.join("src");
    let src_str = src.display().to_string();
    if !src.is_dir() {
        anyhow::bail!("no src/ directory at '{}' - nothing to format", src_str);
    }

    let mut tools = Vec::new();
    let mut broken = 0usize;
    for (bin, hint) in [
        ("stylua", "install with: aftman add JohnnyMorganz/StyLua"),
        ("selene", "install with: aftman add Kampfkarren/selene"),
    ] {
        if !crate::check::on_path(bin) {
            tools.push(ToolRow {
                name: bin,
                ok: false,
                summary: String::new(),
                hint: Some(hint),
            });
            broken += 1;
            continue;
        }
        let tool_args: Vec<String> = if bin == "stylua" {
            if args.check {
                vec!["--check".into(), src_str.clone()]
            } else {
                vec![src_str.clone()]
            }
        } else {
            vec![src_str.clone()]
        };
        let refs: Vec<&str> = tool_args.iter().map(String::as_str).collect();
        let out = crate::tool::run(bin, &refs, root)?;
        let ok = out.code == 0;
        if !ok {
            broken += 1;
        }
        let summary = crate::tool::summary(&out);
        let summary = if ok && summary.is_empty() {
            "ok".to_string()
        } else {
            summary
        };
        tools.push(ToolRow {
            name: bin,
            ok,
            summary,
            hint: None,
        });
    }

    let report = FmtReport {
        path: root.display().to_string(),
        check: args.check,
        tools,
        broken,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(report)
}

fn print_report(r: &FmtReport) {
    let mode = if r.check { "check" } else { "fix" };
    println!("  {}  {} ({})", "esmeril fmt".bold().cyan(), r.path, mode);
    println!();
    for t in &r.tools {
        let mark = if t.ok { "✓".green() } else { "✗".red() };
        if t.ok {
            println!("  {} {:<8} {}", mark, t.name, t.summary.dimmed());
        } else if let Some(hint) = t.hint {
            println!("  {} {:<8} missing · {}", mark, t.name, hint.dimmed());
        } else {
            println!("  {} {:<8} {}", mark, t.name, t.summary.red());
        }
    }
    println!();
    let summary = format!("  {} tools · {} broken", r.tools.len(), r.broken);
    if r.broken > 0 {
        println!("{}", summary.red().bold());
    } else {
        println!("{}", summary.green());
    }
}
