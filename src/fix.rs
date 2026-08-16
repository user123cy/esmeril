use std::path::Path;

use crate::template;

pub struct FixResult {
    pub created: Vec<String>,
    pub needs_attention: Vec<String>,
}

pub fn apply(root: &Path) -> anyhow::Result<FixResult> {
    let name = project_name(root);
    let lib = is_lib(root);
    let project_exists = root.join("default.project.json").exists();
    let files = template::files(&name, false, lib);

    let mut created = Vec::new();
    let mut needs_attention = Vec::new();
    for (rel, content) in &files {
        if project_exists && (rel == "default.project.json" || rel.starts_with("src/")) {
            continue;
        }
        let path = root.join(rel);
        if path.exists() {
            if config_invalid(&path) {
                needs_attention.push(rel.clone());
            }
            continue;
        }
        template::write_one(root, rel, content)?;
        created.push(rel.clone());
    }

    let mut paths = crate::check::project_paths(root);
    paths.sort();
    paths.dedup();
    for p in paths {
        let dir = root.join(&p);
        if !dir.is_dir() {
            let entry = format!("{p}/init.lua");
            template::write_one(root, &entry, "return {}\n")?;
            created.push(entry);
        }
    }

    Ok(FixResult {
        created,
        needs_attention,
    })
}

fn project_name(root: &Path) -> String {
    if let Ok(text) = std::fs::read_to_string(root.join("default.project.json"))
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(name) = value.get("name").and_then(|n| n.as_str())
    {
        return name.to_string();
    }
    root.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("game")
        .to_string()
}

pub(crate) fn is_lib(root: &Path) -> bool {
    if let Ok(text) = std::fs::read_to_string(root.join("default.project.json"))
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(tree) = value.get("tree").and_then(|t| t.as_object())
        && tree.len() == 1
        && tree.contains_key("$path")
    {
        return true;
    }
    root.join("src/init.lua").exists() && !root.join("src/server").exists()
}

fn config_invalid(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    match name {
        "default.project.json" | ".luaurc" => {
            serde_json::from_str::<serde_json::Value>(&text).is_err()
        }
        ".selene.toml" | "stylua.toml" | "aftman.toml" | "wally.toml" => {
            toml::from_str::<toml::Value>(&text).is_err()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("esmeril-fix-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn fix_upgrades_a_bare_dir_to_an_a_grade() {
        let dir = temp_root("bare");
        std::fs::write(dir.join("README.md"), "hello").unwrap();
        let fix = apply(&dir).unwrap();
        assert!(!fix.created.is_empty());
        assert!(fix.needs_attention.is_empty());
        let report = crate::check::inspect(&dir);
        assert_eq!(report.score, 100);
        assert_eq!(report.grade, 'A');
    }

    #[test]
    fn fix_creates_missing_path_dirs() {
        let dir = temp_root("paths");
        std::fs::write(
            dir.join("default.project.json"),
            r#"{"name": "g", "tree": { "a": { "$path": "src/custom" } }}"#,
        )
        .unwrap();
        let fix = apply(&dir).unwrap();
        assert!(fix.created.contains(&"src/custom/init.lua".to_string()));
        assert!(dir.join("src/custom/init.lua").exists());
    }

    #[test]
    fn fix_reports_invalid_configs_without_touching_them() {
        let dir = temp_root("invalid");
        std::fs::write(dir.join(".selene.toml"), "not toml =").unwrap();
        let fix = apply(&dir).unwrap();
        assert!(fix.needs_attention.contains(&".selene.toml".to_string()));
        assert!(!fix.created.contains(&".selene.toml".to_string()));
        assert_eq!(
            std::fs::read_to_string(dir.join(".selene.toml")).unwrap(),
            "not toml ="
        );
    }

    #[test]
    fn fix_is_idempotent() {
        let dir = temp_root("idem");
        apply(&dir).unwrap();
        let fix = apply(&dir).unwrap();
        assert!(fix.created.is_empty());
        assert!(fix.needs_attention.is_empty());
    }
}
