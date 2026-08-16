use std::path::Path;

use colored::Colorize;
use serde::Serialize;

use crate::cli::CheckArgs;

#[derive(Serialize)]
pub struct Report {
    pub path: String,
    pub score: u8,
    pub grade: char,
    pub checks: Vec<CheckRow>,
    pub info: Vec<InfoRow>,
}

#[derive(Debug, Serialize)]
pub struct CheckRow {
    pub label: &'static str,
    pub weight: u8,
    pub ok: bool,
    pub detail: Option<String>,
    pub fix: Option<&'static str>,
}

#[derive(Serialize)]
pub struct InfoRow {
    pub label: String,
    pub ok: bool,
}

impl Report {
    pub fn fail(&self) -> bool {
        // A project that cannot be built by Rojo fails CI regardless of grade.
        if self
            .checks
            .iter()
            .any(|c| matches!(c.label, "default.project.json" | "project paths") && !c.ok)
        {
            return true;
        }
        matches!(self.grade, 'D' | 'F')
    }
}

pub fn run(args: &CheckArgs, json: bool) -> anyhow::Result<Report> {
    let root = Path::new(&args.path);
    let report = inspect(root);
    if args.fix {
        let fix = crate::fix::apply(root)?;
        let after = inspect(root);
        if json {
            println!("{}", serde_json::to_string_pretty(&after)?);
        } else if args.markdown {
            print_markdown(&after);
        } else {
            print_report(&report);
            println!();
            println!("  fixing");
            for created in &fix.created {
                println!("  {} {} {}", "✓".green(), created, "created".dimmed());
            }
            for broken in &fix.needs_attention {
                println!(
                    "  {} {} {}",
                    "✗".red(),
                    broken,
                    "exists but invalid - remove it and run --fix again".dimmed()
                );
            }
            if fix.created.is_empty() && fix.needs_attention.is_empty() {
                println!("  nothing to fix");
            }
            println!();
            println!("  now   {}", colored_grade(after.grade, after.score));
        }
        return Ok(after);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if args.markdown {
        print_markdown(&report);
    } else {
        print_report(&report);
    }
    Ok(report)
}

pub fn inspect(root: &Path) -> Report {
    let mut checks = Vec::new();
    let mut score = 0u8;
    let mut push = |label: &'static str,
                    weight: u8,
                    ok: bool,
                    detail: Option<String>,
                    fix: Option<&'static str>| {
        if ok {
            score += weight;
        }
        checks.push(CheckRow {
            label,
            weight,
            ok,
            detail,
            fix,
        });
    };

    let project_text = std::fs::read_to_string(root.join("default.project.json")).ok();
    let project_value = project_text
        .as_deref()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok());
    let name_ok = project_value
        .as_ref()
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .is_some();
    let paths = project_paths(root);
    let paths_ok = !paths.is_empty() && paths.iter().all(|p| root.join(p).exists());
    let paths_detail = (!paths.is_empty()).then(|| paths.join(", "));

    push(
        "default.project.json",
        15,
        project_value.is_some() && name_ok,
        None,
        Some("add default.project.json with a name and a tree"),
    );
    push(
        "project paths",
        10,
        paths_ok,
        paths_detail,
        Some("create the directories referenced by $path"),
    );
    push(
        "src/",
        10,
        root.join("src").is_dir(),
        None,
        Some("create src/ for Rojo to sync"),
    );
    push(
        ".luaurc",
        10,
        parses_json(&root.join(".luaurc")),
        None,
        Some("add .luaurc with languageMode NonStrict or Strict"),
    );
    push(
        ".selene.toml",
        15,
        parses_toml(&root.join(".selene.toml")),
        None,
        Some("add .selene.toml with std = \"roblox\""),
    );
    push(
        "stylua.toml",
        15,
        parses_toml(&root.join("stylua.toml")),
        None,
        Some("add stylua.toml"),
    );
    push(
        "aftman.toml",
        15,
        aftman_ok(root),
        None,
        Some("add aftman.toml with rojo, selene and stylua under [tools]"),
    );
    push(
        "wally.toml",
        5,
        parses_toml(&root.join("wally.toml")),
        None,
        Some("add wally.toml"),
    );
    push(
        ".github/workflows",
        5,
        has_ci(root),
        None,
        Some("add a CI workflow, e.g. with Roblox/setup-aftman-action"),
    );

    let mut info = Vec::new();
    info.push(InfoRow {
        label: "README.md".into(),
        ok: root.join("README.md").exists(),
    });
    info.push(InfoRow {
        label: ".gitignore".into(),
        ok: root.join(".gitignore").exists(),
    });
    for tool in ["rojo", "selene", "stylua"] {
        info.push(InfoRow {
            label: format!("{tool} on PATH"),
            ok: on_path(tool),
        });
    }

    Report {
        path: root.display().to_string(),
        score,
        grade: grade(score),
        checks,
        info,
    }
}

pub(crate) fn project_paths(root: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(root.join("default.project.json")).ok();
    text.as_deref()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok())
        .and_then(|v| v.get("tree").cloned())
        .map(|tree| collect_paths(&tree))
        .unwrap_or_default()
}

fn render_markdown(r: &Report) -> String {
    let mut out = String::new();
    out.push_str("| check | weight | status |\n");
    out.push_str("|---|---|---|\n");
    for c in &r.checks {
        let status = if c.ok { "✓" } else { "✗" };
        out.push_str(&format!("| {} | {} | {} |\n", c.label, c.weight, status));
    }
    out.push_str(&format!("\n**grade: {} ({}/100)**\n", r.grade, r.score));
    out
}

fn print_markdown(r: &Report) {
    print!("{}", render_markdown(r));
}

fn collect_paths(value: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    match value {
        serde_json::Value::Object(map) => {
            if let Some(p) = map.get("$path").and_then(|v| v.as_str()) {
                out.push(p.to_string());
            }
            for v in map.values() {
                out.extend(collect_paths(v));
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                out.extend(collect_paths(v));
            }
        }
        _ => {}
    }
    out
}

fn aftman_ok(root: &Path) -> bool {
    let text = match std::fs::read_to_string(root.join("aftman.toml")) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let value: toml::Value = match toml::from_str(&text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let tools = match value.get("tools").and_then(|t| t.as_table()) {
        Some(t) => t,
        None => return false,
    };
    tools.contains_key("rojo") && tools.contains_key("selene") && tools.contains_key("stylua")
}

fn parses_toml(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|t| toml::from_str::<toml::Value>(&t).is_ok())
        .unwrap_or(false)
}

fn parses_json(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|t| serde_json::from_str::<serde_json::Value>(&t).is_ok())
        .unwrap_or(false)
}

fn has_ci(root: &Path) -> bool {
    match std::fs::read_dir(root.join(".github").join("workflows")) {
        Ok(mut entries) => entries.next().is_some(),
        Err(_) => false,
    }
}

pub(crate) fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let exe = if cfg!(windows) {
                    format!("{tool}.exe")
                } else {
                    tool.to_string()
                };
                dir.join(&exe).exists()
            })
        })
        .unwrap_or(false)
}

fn grade(score: u8) -> char {
    match score {
        90..=100 => 'A',
        75..=89 => 'B',
        60..=74 => 'C',
        45..=59 => 'D',
        _ => 'F',
    }
}

pub(crate) fn print_report(r: &Report) {
    println!("  {}  {}", "esmeril check".bold().cyan(), r.path);
    println!();
    println!("  tooling");
    for c in &r.checks {
        let mark = if c.ok { "✓".green() } else { "✗".red() };
        let line = format!("  {} {:<20}", mark, c.label);
        if c.ok {
            match &c.detail {
                Some(d) => println!("{} {}", line, d.dimmed()),
                None => println!("{} ok", line),
            }
        } else if let Some(fix) = c.fix {
            println!("{} missing · {}", line, fix);
        } else {
            println!("{} missing", line);
        }
    }
    println!();
    println!("  info");
    for i in &r.info {
        let mark = if i.ok { "✓".green() } else { "✗".red() };
        println!("  {} {}", mark, i.label);
    }
    println!();
    println!("  grade  {}", colored_grade(r.grade, r.score));
}

pub(crate) fn colored_grade(g: char, score: u8) -> String {
    let s = format!("{g}  ({score}/100)");
    match g {
        'A' | 'B' => s.green().bold().to_string(),
        'C' => s.yellow().bold().to_string(),
        _ => s.red().bold().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template;
    use std::path::PathBuf;

    fn write_template(root: &Path) {
        let files = template::files("testgame", false, false);
        for (rel, content) in &files {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("esmeril-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn grade_boundaries() {
        assert_eq!(grade(100), 'A');
        assert_eq!(grade(90), 'A');
        assert_eq!(grade(89), 'B');
        assert_eq!(grade(75), 'B');
        assert_eq!(grade(74), 'C');
        assert_eq!(grade(60), 'C');
        assert_eq!(grade(59), 'D');
        assert_eq!(grade(45), 'D');
        assert_eq!(grade(44), 'F');
        assert_eq!(grade(0), 'F');
    }

    #[test]
    fn fresh_template_scores_full() {
        let dir = temp_root("template");
        write_template(&dir);
        let r = inspect(&dir);
        assert_eq!(r.score, 100, "{:?}", r.checks);
        assert_eq!(r.grade, 'A');
        assert!(!r.fail());
    }

    #[test]
    fn empty_dir_scores_zero() {
        let dir = temp_root("empty");
        let r = inspect(&dir);
        assert_eq!(r.score, 0);
        assert_eq!(r.grade, 'F');
        assert!(r.fail());
    }

    #[test]
    fn missing_src_fails_paths_and_src() {
        let dir = temp_root("nopath");
        write_template(&dir);
        std::fs::remove_dir_all(dir.join("src")).unwrap();
        let r = inspect(&dir);
        let paths = r
            .checks
            .iter()
            .find(|c| c.label == "project paths")
            .unwrap();
        assert!(!paths.ok);
        let src = r.checks.iter().find(|c| c.label == "src/").unwrap();
        assert!(!src.ok);
        assert_eq!(r.score, 80);
        assert!(r.fail());
    }

    #[test]
    fn broken_project_json_fails_parse_and_paths() {
        let dir = temp_root("broken");
        write_template(&dir);
        std::fs::write(dir.join("default.project.json"), "{ not json").unwrap();
        let r = inspect(&dir);
        let project = r
            .checks
            .iter()
            .find(|c| c.label == "default.project.json")
            .unwrap();
        assert!(!project.ok);
        let paths = r
            .checks
            .iter()
            .find(|c| c.label == "project paths")
            .unwrap();
        assert!(!paths.ok);
    }

    #[test]
    fn missing_aftman_tool_fails() {
        let dir = temp_root("noaftman");
        write_template(&dir);
        std::fs::write(
            dir.join("aftman.toml"),
            "[tools]\nrojo = \"rojo-rbx/rojo@7.7.0\"\n",
        )
        .unwrap();
        let r = inspect(&dir);
        let aftman = r.checks.iter().find(|c| c.label == "aftman.toml").unwrap();
        assert!(!aftman.ok);
        assert_eq!(r.score, 85);
    }

    #[test]
    fn recommended_missing_keeps_a_grade() {
        let dir = temp_root("norecs");
        write_template(&dir);
        std::fs::remove_file(dir.join("wally.toml")).unwrap();
        std::fs::remove_dir_all(dir.join(".github")).unwrap();
        let r = inspect(&dir);
        assert_eq!(r.score, 90);
        assert_eq!(r.grade, 'A');
    }

    #[test]
    fn markdown_renders_table() {
        let dir = temp_root("markdown");
        write_template(&dir);
        let r = inspect(&dir);
        let md = render_markdown(&r);
        assert!(md.contains("| default.project.json | 15 | ✓ |"));
        assert!(md.contains("**grade: A (100/100)**"));
    }

    #[test]
    fn collect_paths_nested() {
        let value = serde_json::json!({
            "a": { "$path": "src/a" },
            "b": { "c": { "$path": "src/b/c" } }
        });
        assert_eq!(collect_paths(&value), vec!["src/a", "src/b/c"]);
    }
}
