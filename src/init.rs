use std::path::{Path, PathBuf};

use colored::Colorize;

use crate::cli::InitArgs;
use crate::template;

pub fn run(args: &InitArgs) -> anyhow::Result<()> {
    let target = resolve_target(args);
    if !args.force && !dir_empty(&target) {
        anyhow::bail!(
            "'{}' is not empty; use --force to overwrite",
            target.display()
        );
    }

    let name = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("game")
        .to_string();

    let files = template::files(&name, args.strict, args.lib);
    template::write_all(&target, &files)?;

    println!(
        "  {} {} {}",
        "scaffolded".green().bold(),
        name,
        "in".dimmed()
    );
    println!("  {}", target.display().to_string().dimmed());
    println!();
    for rel in files.keys() {
        println!("  {} {}", "created".green(), rel);
    }
    println!();
    if args.lib {
        println!(
            "  next:  aftman install && wally publish (set your real scope in wally.toml first)"
        );
    } else {
        println!("  next:  aftman install && rojo serve");
    }
    println!("  check: esmeril check");
    Ok(())
}

fn resolve_target(args: &InitArgs) -> PathBuf {
    match &args.name {
        Some(name) => PathBuf::from(name),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

fn dir_empty(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check;

    #[test]
    fn init_produces_an_a_project() {
        let dir = temp_root("init-a");
        run(&InitArgs {
            name: Some(dir.to_str().unwrap().to_string()),
            strict: false,
            lib: false,
            force: false,
        })
        .unwrap();
        let report = check::inspect(&dir);
        assert_eq!(report.score, 100);
        assert_eq!(report.grade, 'A');
        assert!(!report.fail());
    }

    #[test]
    fn init_lib_produces_an_a_project() {
        let dir = temp_root("init-lib");
        run(&InitArgs {
            name: Some(dir.to_str().unwrap().to_string()),
            strict: false,
            lib: true,
            force: false,
        })
        .unwrap();
        let report = check::inspect(&dir);
        assert_eq!(report.score, 100);
        assert_eq!(report.grade, 'A');
        assert!(dir.join("src/init.lua").exists());
        assert!(!dir.join("src/server").exists());
    }

    #[test]
    fn init_strict_sets_language_mode() {
        let dir = temp_root("init-strict");
        run(&InitArgs {
            name: Some(dir.to_str().unwrap().to_string()),
            strict: true,
            lib: false,
            force: false,
        })
        .unwrap();
        let luaurc = std::fs::read_to_string(dir.join(".luaurc")).unwrap();
        assert!(luaurc.contains("Strict"));
    }

    #[test]
    fn init_kebab_cases_wally_name() {
        let dir = temp_root("init-slug");
        let target = dir.join("My Game");
        run(&InitArgs {
            name: Some(target.to_str().unwrap().to_string()),
            strict: false,
            lib: false,
            force: false,
        })
        .unwrap();
        let wally = std::fs::read_to_string(target.join("wally.toml")).unwrap();
        assert!(wally.contains("name = \"user/my-game\""), "{wally}");
        let report = check::inspect(&target);
        assert_eq!(report.score, 100);
        assert_eq!(report.grade, 'A');
    }

    #[test]
    fn init_refuses_non_empty_dir() {
        let dir = temp_root("init-nonempty");
        std::fs::write(dir.join("existing.txt"), "x").unwrap();
        let err = run(&InitArgs {
            name: Some(dir.to_str().unwrap().to_string()),
            strict: false,
            lib: false,
            force: false,
        })
        .unwrap_err();
        assert!(err.to_string().contains("--force"));
    }

    #[test]
    fn init_force_overwrites() {
        let dir = temp_root("init-force");
        std::fs::write(dir.join("existing.txt"), "x").unwrap();
        run(&InitArgs {
            name: Some(dir.to_str().unwrap().to_string()),
            strict: false,
            lib: false,
            force: true,
        })
        .unwrap();
        assert!(dir.join("default.project.json").exists());
    }

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("esmeril-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
