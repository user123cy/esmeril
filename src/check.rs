use std::path::Path;

use colored::Colorize;
use serde::Serialize;

use crate::cli::CheckArgs;

#[derive(Debug, Serialize)]
pub struct Report {
    pub path: String,
    pub score: u8,
    pub grade: char,
    pub checks: Vec<CheckRow>,
    pub info: Vec<InfoRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixSummary>,
}

#[derive(Debug, Serialize)]
pub struct FixSummary {
    pub created: Vec<String>,
    pub needs_attention: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckRow {
    pub label: &'static str,
    pub weight: u8,
    pub ok: bool,
    /// ok | missing | invalid | broken
    pub state: &'static str,
    pub detail: Option<String>,
    pub fix: Option<&'static str>,
}

#[derive(Debug, Serialize)]
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
    if !args.fix && !root.exists() {
        anyhow::bail!(
            "path '{}' does not exist (use --fix to scaffold it)",
            args.path
        );
    }
    let report = inspect(root);
    if args.fix {
        let fix = crate::fix::apply(root)?;
        let mut after = inspect(root);
        after.fix = Some(FixSummary {
            created: fix.created.clone(),
            needs_attention: fix.needs_attention.clone(),
        });
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
                    state: &'static str,
                    detail: Option<String>,
                    fix: Option<&'static str>| {
        let ok = state == "ok";
        if ok {
            score += weight;
        }
        checks.push(CheckRow {
            label,
            weight,
            ok,
            state,
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
    let project_state = if project_text.is_none() {
        "missing"
    } else if project_value.is_some() && name_ok {
        "ok"
    } else {
        "invalid"
    };
    let paths_state = if project_value.is_none() {
        if project_text.is_none() {
            "missing"
        } else {
            "invalid"
        }
    } else if paths_ok {
        "ok"
    } else {
        "broken"
    };

    push(
        "default.project.json",
        15,
        project_state,
        None,
        Some("add default.project.json with a name and a tree"),
    );
    push(
        "project paths",
        10,
        paths_state,
        paths_detail,
        Some("create the directories referenced by $path"),
    );
    push(
        "src/",
        10,
        if root.join("src").is_dir() {
            "ok"
        } else {
            "missing"
        },
        None,
        Some("create src/ for Rojo to sync"),
    );
    let luaurc = root.join(".luaurc");
    let luaurc_state = if !luaurc.exists() {
        "missing"
    } else if luaurc_ok(root) {
        "ok"
    } else if parses_json(&luaurc) {
        "broken"
    } else {
        "invalid"
    };
    push(
        ".luaurc",
        10,
        luaurc_state,
        None,
        Some("add .luaurc with languageMode NonStrict or Strict"),
    );
    push(
        ".selene.toml",
        15,
        toml_state(&root.join(".selene.toml")),
        None,
        Some("add .selene.toml with std = \"roblox\""),
    );
    push(
        "stylua.toml",
        15,
        toml_state(&root.join("stylua.toml")),
        None,
        Some("add stylua.toml"),
    );
    let aftman = root.join("aftman.toml");
    let aftman_state = if !aftman.exists() {
        "missing"
    } else if aftman_ok(root) {
        "ok"
    } else if parses_toml(&aftman) {
        "broken"
    } else {
        "invalid"
    };
    push(
        "aftman.toml",
        15,
        aftman_state,
        None,
        Some("add aftman.toml with rojo, selene and stylua as owner/repo@tag under [tools]"),
    );
    let wally = root.join("wally.toml");
    let wally_state = if !wally.exists() {
        "missing"
    } else if wally_ok(root) {
        "ok"
    } else if parses_toml(&wally) {
        "broken"
    } else {
        "invalid"
    };
    push(
        "wally.toml",
        5,
        wally_state,
        None,
        Some("add wally.toml with [package] name, version, registry and realm"),
    );
    push(
        ".github/workflows",
        5,
        if has_ci(root) { "ok" } else { "missing" },
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
        fix: None,
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
    let info: Vec<String> = r
        .info
        .iter()
        .map(|i| format!("{} {}", if i.ok { "✓" } else { "✗" }, i.label))
        .collect();
    out.push_str(&format!("info: {}\n", info.join(", ")));
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
    ["rojo", "selene", "stylua"].iter().all(|name| {
        tools
            .get(*name)
            .and_then(|v| v.as_str())
            .is_some_and(valid_tool_spec)
    })
}

// Aftman pins are "owner/repo@tag"; a trailing ".exe" is allowed for Windows tools.
fn valid_tool_spec(spec: &str) -> bool {
    let Some((name, tag)) = spec.rsplit_once('@') else {
        return false;
    };
    if tag.is_empty() {
        return false;
    }
    let name = name.strip_suffix(".exe").unwrap_or(name);
    let mut parts = name.split('/');
    let owner = parts.next().unwrap_or("");
    let repo = parts.next().unwrap_or("");
    !owner.is_empty() && !repo.is_empty() && parts.next().is_none()
}

// A wally.toml only earns its points when [package] is complete enough for wally itself.
fn wally_ok(root: &Path) -> bool {
    let text = match std::fs::read_to_string(root.join("wally.toml")) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let value: toml::Value = match toml::from_str(&text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let Some(pkg) = value.get("package").and_then(|p| p.as_table()) else {
        return false;
    };
    let name_ok = pkg.get("name").and_then(|n| n.as_str()).is_some_and(|n| {
        let mut parts = n.split('/');
        let scope = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        !scope.is_empty() && !name.is_empty() && parts.next().is_none()
    });
    let version_ok = pkg
        .get("version")
        .and_then(|v| v.as_str())
        .and_then(crate::deps::parse_version)
        .is_some();
    let registry_ok = pkg
        .get("registry")
        .and_then(|r| r.as_str())
        .is_some_and(|r| r.starts_with("https://"));
    let realm_ok = pkg
        .get("realm")
        .and_then(|r| r.as_str())
        .is_some_and(|r| matches!(r, "shared" | "server" | "client"));
    name_ok && version_ok && registry_ok && realm_ok
}

fn parses_toml(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|t| toml::from_str::<toml::Value>(&t).is_ok())
        .unwrap_or(false)
}

fn toml_state(path: &Path) -> &'static str {
    if !path.exists() {
        "missing"
    } else if parses_toml(path) {
        "ok"
    } else {
        "invalid"
    }
}

fn luaurc_ok(root: &Path) -> bool {
    let text = match std::fs::read_to_string(root.join(".luaurc")) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    let Some(lang) = value.get("languageMode") else {
        return false;
    };
    // languageMode is usually an object mapping paths to modes, but a plain
    // string is accepted too. Only NonStrict and Strict are valid Luau modes.
    let modes: Vec<&serde_json::Value> = match lang {
        serde_json::Value::String(_) => vec![lang],
        serde_json::Value::Object(map) => map.values().collect(),
        _ => return false,
    };
    !modes.is_empty()
        && modes
            .iter()
            .all(|m| matches!(m.as_str(), Some("NonStrict" | "Strict")))
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
            let verb = match c.state {
                "missing" => "missing",
                "invalid" => "invalid",
                _ => "broken",
            };
            match &c.detail {
                Some(d) => println!("{} {} · {} ({})", line, verb, fix, d),
                None => println!("{} {} · {}", line, verb, fix),
            }
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
    fn luaurc_without_language_mode_is_broken() {
        let dir = temp_root("luaurc");
        write_template(&dir);
        std::fs::write(dir.join(".luaurc"), "{}\n").unwrap();
        let r = inspect(&dir);
        let luaurc = r.checks.iter().find(|c| c.label == ".luaurc").unwrap();
        assert!(!luaurc.ok);
        assert_eq!(luaurc.state, "broken");
        assert_eq!(r.score, 90);
    }

    #[test]
    fn states_distinguish_missing_invalid_broken() {
        let dir = temp_root("states");
        write_template(&dir);
        std::fs::write(dir.join("default.project.json"), "{ not json").unwrap();
        let r = inspect(&dir);
        let project = r
            .checks
            .iter()
            .find(|c| c.label == "default.project.json")
            .unwrap();
        assert_eq!(project.state, "invalid");
        let paths = r
            .checks
            .iter()
            .find(|c| c.label == "project paths")
            .unwrap();
        assert_eq!(paths.state, "invalid");
        std::fs::write(dir.join(".selene.toml"), "not toml =").unwrap();
        let r = inspect(&dir);
        let selene = r.checks.iter().find(|c| c.label == ".selene.toml").unwrap();
        assert_eq!(selene.state, "invalid");
        std::fs::remove_file(dir.join(".selene.toml")).unwrap();
        let r = inspect(&dir);
        let selene = r.checks.iter().find(|c| c.label == ".selene.toml").unwrap();
        assert_eq!(selene.state, "missing");
    }

    #[test]
    fn luaurc_with_invalid_mode_is_broken() {
        let dir = temp_root("luaurc-mode");
        write_template(&dir);
        std::fs::write(
            dir.join(".luaurc"),
            "{\"languageMode\": {\"src/\": \"Typo\"}}\n",
        )
        .unwrap();
        let r = inspect(&dir);
        let luaurc = r.checks.iter().find(|c| c.label == ".luaurc").unwrap();
        assert!(!luaurc.ok);
        assert_eq!(luaurc.state, "broken");
    }

    #[test]
    fn aftman_bad_spec_format_is_broken() {
        let dir = temp_root("aftman-format");
        write_template(&dir);
        std::fs::write(
            dir.join("aftman.toml"),
            "[tools]\nrojo = \"not-a-spec\"\nselene = \"Kampfkarren/selene@0.31.0\"\nstylua = \"JohnnyMorganz/StyLua@2.5.0\"\n",
        )
        .unwrap();
        let r = inspect(&dir);
        let aftman = r.checks.iter().find(|c| c.label == "aftman.toml").unwrap();
        assert!(!aftman.ok);
        assert_eq!(aftman.state, "broken");
    }

    #[test]
    fn wally_without_package_is_broken() {
        let dir = temp_root("wally-pkg");
        write_template(&dir);
        std::fs::write(dir.join("wally.toml"), "[dependencies]\n").unwrap();
        let r = inspect(&dir);
        let wally = r.checks.iter().find(|c| c.label == "wally.toml").unwrap();
        assert!(!wally.ok);
        assert_eq!(wally.state, "broken");
        assert_eq!(r.score, 95);
    }

    #[test]
    fn check_bails_on_missing_path() {
        let dir = temp_root("missing-path");
        let _ = std::fs::remove_dir_all(&dir);
        let err = run(
            &CheckArgs {
                path: dir.to_str().unwrap().to_string(),
                fix: false,
                markdown: false,
            },
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("does not exist"));
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
        assert!(md.contains("info: ✓ README.md, ✓ .gitignore"));
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
