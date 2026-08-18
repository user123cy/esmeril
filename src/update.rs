use std::path::Path;

use colored::Colorize;
use serde::Serialize;
use toml_edit::{DocumentMut, Value};

use crate::cli::UpdateArgs;
use crate::deps;

#[derive(Serialize)]
pub struct UpdateReport {
    pub path: String,
    pub written: bool,
    pub updates: Vec<UpdateRow>,
    pub up_to_date: usize,
    pub errors: usize,
}

#[derive(Serialize)]
pub struct UpdateRow {
    pub section: String,
    pub name: String,
    pub required: String,
    pub latest: String,
}

impl UpdateReport {
    pub fn fail(&self) -> bool {
        self.errors > 0
    }
}

pub fn run(args: &UpdateArgs, json: bool) -> anyhow::Result<UpdateReport> {
    let root = Path::new(&args.path);
    let path = root.join("wally.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("failed to read '{}': {}", path.display(), e))?;
    let base = deps::raw_base(&deps::registry(root)?)?;
    let agent = deps::agent();

    let mut updates = Vec::new();
    let mut up_to_date = 0usize;
    let mut errors = 0usize;
    for (section, alias, spec_value) in &deps::parse_specs(&text) {
        let required = match spec_value {
            toml::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let target = deps::resolve(alias, spec_value);
        if !json && matches!(&target, deps::Target::Registry { .. }) {
            eprintln!("  checking {} ({section}) ...", alias.dimmed());
        }
        let deps::Target::Registry { name, req } = target else {
            continue;
        };
        let url = format!("{base}/{name}");
        let manifest = match crate::cache::fetch_cached(&agent, &url, args.offline) {
            Ok(t) => t,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let versions = deps::parse_manifest(&manifest);
        match deps::latest_stable(&versions) {
            Some(latest) if !req.satisfies(&latest) => {
                updates.push(UpdateRow {
                    section: section.clone(),
                    name: alias.clone(),
                    required,
                    latest: format!("{name}@{}", latest),
                });
            }
            Some(_) => up_to_date += 1,
            None => errors += 1,
        }
    }

    let mut written = false;
    if args.write && !updates.is_empty() {
        let new_text = apply_updates(&text, &updates)?;
        std::fs::write(&path, new_text)
            .map_err(|e| anyhow::anyhow!("failed to write '{}': {}", path.display(), e))?;
        written = true;
    }

    let report = UpdateReport {
        path: root.display().to_string(),
        written,
        updates,
        up_to_date,
        errors,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(report)
}

fn apply_updates(text: &str, updates: &[UpdateRow]) -> anyhow::Result<String> {
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid wally.toml: {e}"))?;
    for u in updates {
        let version = u
            .latest
            .rsplit_once('@')
            .map(|(_, v)| v)
            .unwrap_or(&u.latest);
        let table = doc
            .get_mut(&u.section)
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| anyhow::anyhow!("missing section [{}]", u.section))?;
        let item = table
            .get_mut(&u.name)
            .ok_or_else(|| anyhow::anyhow!("missing dependency '{}'", u.name))?;
        let toml_edit::Item::Value(v) = item else {
            continue;
        };
        match v {
            toml_edit::Value::String(s) => {
                let raw = s.value();
                if raw.contains('@') {
                    let (name, _) = raw.rsplit_once('@').expect("checked above");
                    *s = toml_edit::Formatted::new(format!("{name}@{version}"));
                } else {
                    *s = toml_edit::Formatted::new(version.to_string());
                }
            }
            toml_edit::Value::InlineTable(t) => {
                t.insert("version", Value::from(version.to_string()));
            }
            _ => {}
        }
    }
    Ok(doc.to_string())
}

fn print_report(r: &UpdateReport) {
    println!("  {}  {}", "esmeril update".bold().cyan(), r.path);
    println!();
    if r.updates.is_empty() {
        println!("  everything is up to date");
    } else {
        for u in &r.updates {
            println!(
                "  {} {:<28} {} {} {}",
                u.section.dimmed(),
                u.name,
                u.required.yellow(),
                "→".dimmed(),
                u.latest.green()
            );
        }
    }
    println!();
    let summary = format!(
        "  {} outdated · {} up to date · {} errors",
        r.updates.len(),
        r.up_to_date,
        r.errors
    );
    if r.errors > 0 {
        println!("{}", summary.red().bold());
    } else if r.updates.is_empty() {
        println!("{}", summary.green());
    } else if r.written {
        println!("{} · wally.toml updated", summary.green().bold());
    } else {
        println!("{} · rerun with --write to apply", summary.yellow());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_preserves_comments_and_formatting() {
        let text = r#"[package]
name = "user/game"
version = "0.1.0"

[dependencies]
# pinned to the old release
Promise = "evaera/promise@0.2.1"
Cmdr = { version = "1.8.4" }
"#;
        let updates = vec![
            UpdateRow {
                section: "dependencies".into(),
                name: "Promise".into(),
                required: "evaera/promise@0.2.1".into(),
                latest: "evaera/promise@4.0.0".into(),
            },
            UpdateRow {
                section: "dependencies".into(),
                name: "Cmdr".into(),
                required: "1.8.4".into(),
                latest: "evaera/cmdr@1.12.0".into(),
            },
        ];
        let out = apply_updates(text, &updates).unwrap();
        assert!(out.contains("# pinned to the old release"));
        assert!(out.contains("Promise = \"evaera/promise@4.0.0\""));
        assert!(out.contains("version = \"1.12.0\""));
        assert!(out.contains("name = \"user/game\""));
    }

    #[test]
    fn apply_errors_on_unknown_section() {
        let updates = vec![UpdateRow {
            section: "nope".into(),
            name: "x".into(),
            required: "a/b@1.0.0".into(),
            latest: "a/b@2.0.0".into(),
        }];
        assert!(apply_updates("[package]\nname = \"a/b\"\n", &updates).is_err());
    }
}
