use std::process::Command;

use colored::Colorize;
use serde::Serialize;

use crate::check;

const TOOLS: [&str; 5] = ["rojo", "selene", "stylua", "wally", "aftman"];

// Core tools the workflow depends on; missing ones fail the run.
const CORE: [&str; 3] = ["rojo", "selene", "stylua"];

const HINTS: [(&str, &str); 5] = [
    ("rojo", "install with: aftman add rojo-rbx/rojo"),
    ("selene", "install with: aftman add Kampfkarren/selene"),
    ("stylua", "install with: aftman add JohnnyMorganz/StyLua"),
    ("wally", "install with: aftman add UpliftGames/wally"),
    ("aftman", "install from https://github.com/rojo-rbx/aftman"),
];

#[derive(Serialize)]
pub struct DoctorReport {
    pub tools: Vec<ToolRow>,
    pub ready: usize,
    pub missing: usize,
}

#[derive(Serialize)]
pub struct ToolRow {
    pub name: &'static str,
    pub present: bool,
    pub version: Option<String>,
    pub hint: &'static str,
}

impl DoctorReport {
    pub fn fail(&self) -> bool {
        self.tools
            .iter()
            .any(|t| CORE.contains(&t.name) && !t.present)
    }
}

pub fn run(json: bool) -> anyhow::Result<DoctorReport> {
    let mut tools = Vec::new();
    let mut ready = 0usize;
    let mut missing = 0usize;
    for name in TOOLS {
        let present = check::on_path(name);
        let version = if present { tool_version(name) } else { None };
        let hint = HINTS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, h)| *h)
            .unwrap_or("");
        if present {
            ready += 1;
        } else {
            missing += 1;
        }
        tools.push(ToolRow {
            name,
            present,
            version,
            hint,
        });
    }

    let report = DoctorReport {
        tools,
        ready,
        missing,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(report)
}

fn tool_version(name: &str) -> Option<String> {
    let output = Command::new(name).arg("--version").output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Some tools (notably on Windows) print the version to stderr.
    let text = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    let line = text.lines().next()?.trim();
    Some(line.to_string())
}

fn print_report(r: &DoctorReport) {
    println!("  {}  local toolchain", "esmeril doctor".bold().cyan());
    println!();
    for t in &r.tools {
        let mark = if t.present {
            "✓".green()
        } else {
            "✗".red()
        };
        let version = t.version.as_deref().unwrap_or("-");
        let line = format!("  {} {:<8} {}", mark, t.name, version.dimmed());
        if t.present {
            println!("{line}");
        } else {
            println!("{}  {}", line, t.hint.dimmed());
        }
    }
    println!();
    let summary = format!(
        "  {} of {} tools ready · {} missing",
        r.ready,
        r.tools.len(),
        r.missing
    );
    if r.fail() {
        println!("{}", summary.red().bold());
    } else {
        println!("{}", summary.green());
    }
}
