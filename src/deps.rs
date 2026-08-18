use std::cmp::Ordering;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use colored::Colorize;
use serde::Serialize;

use crate::cli::DepsArgs;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Option<String>,
}

impl Version {
    fn is_stable(&self) -> bool {
        self.prerelease.is_none()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.prerelease {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then_with(|| match (&self.prerelease, &other.prerelease) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => cmp_pre(a, b),
            })
    }
}

// Semver-style prerelease ordering: numeric dot segments compare numerically.
fn cmp_pre(a: &str, b: &str) -> Ordering {
    let a: Vec<&str> = a.split('.').collect();
    let b: Vec<&str> = b.split('.').collect();
    for i in 0..a.len().max(b.len()) {
        match (a.get(i), b.get(i)) {
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(xn), Ok(yn)) => xn.cmp(&yn),
                    _ => x.cmp(y),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (None, None) => unreachable!(),
        }
    }
    Ordering::Equal
}

pub(crate) fn parse_version(raw: &str) -> Option<Version> {
    let raw = raw.trim();
    let (core, prerelease) = match raw.split_once('-') {
        Some((c, p)) => (c, Some(p.to_string())),
        None => (raw, None),
    };
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = match parts.next() {
        None => 0,
        Some(s) => s.parse().ok()?,
    };
    let patch = match parts.next() {
        None => 0,
        Some(s) => s.parse().ok()?,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(Version {
        major,
        minor,
        patch,
        prerelease,
    })
}

// Wally treats a bare version as a caret requirement, like Cargo.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Req {
    Exact(Version),
    Caret(Version),
    Tilde(Version),
}

impl Req {
    pub(crate) fn satisfies(&self, v: &Version) -> bool {
        match self {
            Req::Exact(base) => v == base,
            Req::Caret(base) => {
                if v < base {
                    return false;
                }
                if base.major > 0 {
                    v.major == base.major
                } else if base.minor > 0 {
                    v.minor == base.minor
                } else {
                    v.patch == base.patch
                }
            }
            Req::Tilde(base) => v >= base && v.major == base.major && v.minor == base.minor,
        }
    }
}

fn parse_req(raw: &str) -> Option<Req> {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix('^') {
        return parse_version(rest).map(Req::Caret);
    }
    if let Some(rest) = raw.strip_prefix('~') {
        return parse_version(rest).map(Req::Tilde);
    }
    if let Some(rest) = raw.strip_prefix('=') {
        return parse_version(rest).map(Req::Exact);
    }
    parse_version(raw).map(Req::Caret)
}

pub(crate) enum Target {
    Registry { name: String, req: Req },
    Git,
    Unsupported(String),
}

#[derive(Serialize)]
pub struct DepsReport {
    pub path: String,
    pub deps: Vec<DepRow>,
    pub outdated: usize,
    pub broken: usize,
}

#[derive(Serialize)]
pub struct DepRow {
    pub name: String,
    pub section: String,
    pub required: String,
    pub latest: Option<String>,
    pub status: &'static str,
    pub note: Option<String>,
}

impl DepsReport {
    pub fn fail(&self) -> bool {
        self.broken > 0
    }
}

pub fn run(args: &DepsArgs, json: bool) -> anyhow::Result<DepsReport> {
    let root = Path::new(&args.path);
    let specs = load_specs(root)?;
    let base = raw_base(&registry(root)?)?;
    let agent = agent();

    let mut deps = Vec::new();
    let mut outdated = 0usize;
    let mut broken = 0usize;
    for (section, alias, value) in &specs {
        let target = resolve(alias, value);
        if !json && matches!(&target, Target::Registry { .. }) {
            eprintln!("  checking {} ({section}) ...", alias.dimmed());
        }
        let row = audit(&agent, &base, section, alias, value, &target, args.offline);
        match row.status {
            "outdated" => {
                outdated += 1;
                broken += 1;
            }
            "missing" | "not found" | "error" => broken += 1,
            _ => {}
        }
        deps.push(row);
    }

    let report = DepsReport {
        path: root.display().to_string(),
        deps,
        outdated,
        broken,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(report)
}

pub(crate) fn registry(root: &Path) -> anyhow::Result<String> {
    let text = read_wally(root)?;
    let value: toml::Value =
        toml::from_str(&text).map_err(|e| anyhow::anyhow!("invalid wally.toml: {e}"))?;
    value
        .get("package")
        .and_then(|p| p.get("registry"))
        .and_then(|r| r.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("wally.toml has no [package].registry"))
}

fn load_specs(root: &Path) -> anyhow::Result<Vec<(String, String, toml::Value)>> {
    Ok(parse_specs(&read_wally(root)?))
}

fn read_wally(root: &Path) -> anyhow::Result<String> {
    let path = root.join("wally.toml");
    std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("failed to read '{}': {}", path.display(), e))
}

pub(crate) fn parse_specs(text: &str) -> Vec<(String, String, toml::Value)> {
    let value: toml::Value = match toml::from_str(text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for section in ["dependencies", "server-dependencies", "dev-dependencies"] {
        if let Some(table) = value.get(section).and_then(|v| v.as_table()) {
            for (alias, spec) in table {
                out.push((section.to_string(), alias.clone(), spec.clone()));
            }
        }
    }
    // Stable output regardless of the order in wally.toml.
    out.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    out
}

// Dependency values are "scope/name@version" strings or git tables. Plain
// versions are accepted too when the alias already carries the scope.
pub(crate) fn resolve(alias: &str, value: &toml::Value) -> Target {
    match value {
        toml::Value::String(s) => match s.split_once('@') {
            Some((name, req)) => match parse_req(req) {
                Some(req) => Target::Registry {
                    name: name.to_string(),
                    req,
                },
                None => Target::Unsupported(format!("cannot parse version requirement '{req}'")),
            },
            None if parse_version(s).is_some() => {
                if alias.contains('/') {
                    match parse_req(s) {
                        Some(req) => Target::Registry {
                            name: alias.to_string(),
                            req,
                        },
                        None => Target::Unsupported(format!("cannot parse version '{s}'")),
                    }
                } else {
                    Target::Unsupported(format!(
                        "short name without scope; use '{alias}@<version>' or 'scope/{alias}@<version>'"
                    ))
                }
            }
            None => Target::Unsupported(format!("cannot parse '{s}'")),
        },
        toml::Value::Table(t) => {
            if t.contains_key("git") {
                Target::Git
            } else if let Some(version) = t.get("version").and_then(|v| v.as_str()) {
                if alias.contains('/') {
                    match parse_req(version) {
                        Some(req) => Target::Registry {
                            name: alias.to_string(),
                            req,
                        },
                        None => Target::Unsupported(format!("cannot parse version '{version}'")),
                    }
                } else {
                    Target::Unsupported(format!(
                        "short name without scope; use '{alias}@<version>' or 'scope/{alias}@<version>'"
                    ))
                }
            } else {
                Target::Unsupported(format!("unknown dependency form '{value}'"))
            }
        }
        other => Target::Unsupported(format!("unknown dependency form '{other}'")),
    }
}

// Converts a github.com registry URL into the raw.githubusercontent.com base
// where the index manifests live: packages are stored at {scope}/{name}.
pub(crate) fn raw_base(registry: &str) -> anyhow::Result<String> {
    let r = registry.trim_end_matches('/');
    let r = r.strip_suffix(".git").unwrap_or(r);
    let rest = r
        .strip_prefix("https://github.com/")
        .or_else(|| r.strip_prefix("http://github.com/"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "unsupported registry '{registry}': only github.com registries are supported"
            )
        })?;
    let mut parts = rest.splitn(2, '/');
    let owner = parts.next().unwrap_or("");
    let repo = parts.next().unwrap_or("");
    if owner.is_empty() || repo.is_empty() {
        anyhow::bail!("cannot parse registry '{registry}'");
    }
    Ok(format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/main"
    ))
}

pub(crate) fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(10))
        .build()
}

pub(crate) fn fetch(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    match agent.get(url).call() {
        Ok(resp) => resp.into_string().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(code, _)) => Err(format!("status {code}")),
        Err(e) => Err(e.to_string()),
    }
}

// The registry stores one JSON document per published version, one per line.
pub(crate) fn parse_manifest(text: &str) -> Vec<Version> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| {
            v.get("package")?
                .get("version")?
                .as_str()
                .and_then(parse_version)
        })
        .collect()
}

pub(crate) fn latest_stable(versions: &[Version]) -> Option<Version> {
    versions
        .iter()
        .filter(|v| v.is_stable())
        .max()
        .cloned()
        .or_else(|| versions.iter().max().cloned())
}

fn audit(
    agent: &ureq::Agent,
    base: &str,
    section: &str,
    alias: &str,
    value: &toml::Value,
    target: &Target,
    offline: bool,
) -> DepRow {
    let required = match value {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let mut row = DepRow {
        name: alias.to_string(),
        section: section.to_string(),
        required,
        latest: None,
        status: "",
        note: None,
    };
    match target {
        Target::Git => row.status = "git",
        Target::Unsupported(note) => {
            row.status = "unsupported";
            row.note = Some(note.clone());
        }
        Target::Registry { name, req } => {
            let url = format!("{base}/{name}");
            let text = match crate::cache::fetch_cached(agent, &url, offline) {
                Ok(t) => t,
                Err(e) if e.starts_with("status 404") => {
                    row.status = "not found";
                    row.note = Some("package not in registry".into());
                    return row;
                }
                Err(e) => {
                    row.status = "error";
                    row.note = Some(e);
                    return row;
                }
            };
            let versions = parse_manifest(&text);
            let latest = latest_stable(&versions);
            row.latest = latest.as_ref().map(ToString::to_string);
            if latest.as_ref().is_some_and(|l| req.satisfies(l)) {
                row.status = "ok";
            } else if versions.iter().any(|v| req.satisfies(v)) {
                row.status = "outdated";
            } else {
                row.status = "missing";
                let mut all = versions.clone();
                all.sort();
                all.dedup();
                let shown: Vec<String> = all.iter().take(8).map(|v| v.to_string()).collect();
                row.note = if all.len() > 8 {
                    Some(format!(
                        "nothing satisfies the requirement; available: {}, ... ({} published)",
                        shown.join(", "),
                        all.len()
                    ))
                } else {
                    Some(format!(
                        "nothing satisfies the requirement; available: {}",
                        shown.join(", ")
                    ))
                };
            }
        }
    }
    row
}

fn print_report(r: &DepsReport) {
    println!("  {}  {}", "esmeril deps".bold().cyan(), r.path);
    println!();
    if r.deps.is_empty() {
        println!("  no dependencies");
    } else {
        println!(
            "  {:<28} {:<20} {:<28} {:<12} status",
            "package", "section", "required", "latest"
        );
        for d in &r.deps {
            let latest = d.latest.as_deref().unwrap_or("-");
            let status = match d.status {
                "ok" => "ok".green(),
                "outdated" => "outdated".yellow(),
                "git" => "git".dimmed(),
                "unsupported" => "unsupported".yellow(),
                _ => d.status.red(),
            };
            let mut line = format!(
                "  {:<28} {:<20} {:<28} {:<12} {}",
                d.name, d.section, d.required, latest, status
            );
            if let Some(note) = &d.note {
                line.push_str(&format!(" · {}", note.dimmed()));
            }
            println!("{line}");
        }
    }
    println!();
    let summary = format!(
        "  {} deps · {} outdated · {} broken",
        r.deps.len(),
        r.outdated,
        r.broken
    );
    if r.broken > 0 {
        println!("{}", summary.red().bold());
    } else {
        println!("{}", summary.green());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMDR_MANIFEST: &str = r#"{"package":{"name":"evaera/cmdr","version":"1.8.4","registry":"https://github.com/UpliftGames/wally-index","realm":"server"}}
{"package":{"name":"evaera/cmdr","version":"1.9.0","registry":"https://github.com/UpliftGames/wally-index","realm":"shared"}}
{"package":{"name":"evaera/cmdr","version":"1.12.0","registry":"https://github.com/UpliftGames/wally-index","realm":"shared"}}
{"package":{"name":"evaera/cmdr","version":"1.13.0-rc.3","registry":"https://github.com/UpliftGames/wally-index","realm":"server"}}
"#;

    #[test]
    fn version_ordering() {
        assert!(parse_version("1.12.0") < parse_version("1.13.0"));
        assert!(parse_version("1.13.0-rc.3") < parse_version("1.13.0"));
        assert!(parse_version("1.13.0-rc.3") > parse_version("1.12.0"));
        assert!(parse_version("1.13.0-rc.10") > parse_version("1.13.0-rc.3"));
        assert!(parse_version("1.0.0-alpha") < parse_version("1.0.0-beta"));
        assert_eq!(parse_version("1.2"), parse_version("1.2.0"));
    }

    #[test]
    fn version_display() {
        assert_eq!(
            parse_version("1.13.0-rc.3").unwrap().to_string(),
            "1.13.0-rc.3"
        );
        assert_eq!(parse_version("1.12.0").unwrap().to_string(), "1.12.0");
    }

    #[test]
    fn invalid_versions_rejected() {
        assert_eq!(parse_version("abc"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("1.2.x"), None);
    }

    #[test]
    fn req_semantics() {
        let caret = parse_req("1.2.0").unwrap();
        assert!(caret.satisfies(&parse_version("1.2.0").unwrap()));
        assert!(caret.satisfies(&parse_version("1.9.0").unwrap()));
        assert!(!caret.satisfies(&parse_version("2.0.0").unwrap()));
        assert!(!caret.satisfies(&parse_version("1.1.9").unwrap()));

        let caret_zero = parse_req("0.2.1").unwrap();
        assert!(caret_zero.satisfies(&parse_version("0.2.1").unwrap()));
        assert!(caret_zero.satisfies(&parse_version("0.2.9").unwrap()));
        assert!(!caret_zero.satisfies(&parse_version("0.3.0").unwrap()));

        let exact = parse_req("=1.2.0").unwrap();
        assert!(exact.satisfies(&parse_version("1.2.0").unwrap()));
        assert!(!exact.satisfies(&parse_version("1.2.1").unwrap()));

        let tilde = parse_req("~1.2.0").unwrap();
        assert!(tilde.satisfies(&parse_version("1.2.9").unwrap()));
        assert!(!tilde.satisfies(&parse_version("1.3.0").unwrap()));
    }

    #[test]
    fn latest_prefers_stable() {
        let versions = parse_manifest(CMDR_MANIFEST);
        assert_eq!(versions.len(), 4);
        let latest = latest_stable(&versions).unwrap();
        assert_eq!(latest.to_string(), "1.12.0");
    }

    #[test]
    fn manifest_ignores_garbage_lines() {
        let text = format!(
            "not json\n{CMDR_MANIFEST}\n{{\"package\":{{\"name\":\"x\",\"version\":\"oops\"}}}}\n"
        );
        assert_eq!(parse_manifest(&text).len(), 4);
    }

    #[test]
    fn raw_base_conversions() {
        assert_eq!(
            raw_base("https://github.com/UpliftGames/wally-index").unwrap(),
            "https://raw.githubusercontent.com/UpliftGames/wally-index/main"
        );
        assert_eq!(
            raw_base("https://github.com/UpliftGames/wally-index.git/").unwrap(),
            "https://raw.githubusercontent.com/UpliftGames/wally-index/main"
        );
    }

    #[test]
    fn raw_base_rejects_other_hosts() {
        assert!(raw_base("https://gitlab.com/x/y").is_err());
        assert!(raw_base("https://github.com/onlyowner").is_err());
    }

    #[test]
    fn specs_parse_all_sections() {
        let text = r#"
[package]
name = "user/game"
version = "0.1.0"

[dependencies]
Promise = "evaera/promise@0.2.1"
Cmdr = { version = "1.12.0" }
GitDep = { git = "https://github.com/user/repo.git", rev = "abc" }

[server-dependencies]
Fusion = "elttob/fusion@0.3.0"

[dev-dependencies]
TestEZ = "roblox/testez@0.4.2"
"#;
        let specs = parse_specs(text);
        assert_eq!(specs.len(), 5);
        let values: Vec<(&str, &str)> = specs
            .iter()
            .map(|(s, a, _v)| (s.as_str(), a.as_str()))
            .collect();
        assert!(values.contains(&("dependencies", "Promise")));
        assert!(values.contains(&("dependencies", "Cmdr")));
        assert!(values.contains(&("dependencies", "GitDep")));
        assert!(values.contains(&("server-dependencies", "Fusion")));
        assert!(values.contains(&("dev-dependencies", "TestEZ")));
    }

    #[test]
    fn resolve_registry_spec() {
        let value: toml::Value = toml::from_str("s = \"evaera/cmdr@1.8.4\"").unwrap();
        let t = resolve("Cmdr", value.get("s").unwrap());
        match t {
            Target::Registry { name, req } => {
                assert_eq!(name, "evaera/cmdr");
                assert!(req.satisfies(&parse_version("1.8.4").unwrap()));
            }
            _ => panic!("expected registry target"),
        }
    }

    #[test]
    fn resolve_git_and_short_names() {
        let value: toml::Value =
            toml::from_str("g = { git = \"https://github.com/x/y.git\" }").unwrap();
        assert!(matches!(
            resolve("Dep", value.get("g").unwrap()),
            Target::Git
        ));

        let value: toml::Value = toml::from_str("v = \"0.2.1\"").unwrap();
        assert!(matches!(
            resolve("Promise", value.get("v").unwrap()),
            Target::Unsupported(_)
        ));
        assert!(matches!(
            resolve("evaera/promise", value.get("v").unwrap()),
            Target::Registry { .. }
        ));
    }

    #[test]
    fn specs_empty_without_deps() {
        assert!(parse_specs("[package]\nname = \"a/b\"\n").is_empty());
    }

    #[test]
    fn specs_invalid_toml_is_empty() {
        assert!(parse_specs("not toml = =").is_empty());
    }
}
