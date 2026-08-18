# Changelog

## 0.2.0

### Added
- `esmeril check` now validates config content, not just parsing: `.luaurc` needs a `languageMode` of `NonStrict`/`Strict`, `aftman.toml` tools must be `owner/repo@tag` specs, and `wally.toml` needs a complete `[package]` (scoped name, version, https registry and a valid realm).
- `esmeril check` now fails fast with a clear error when the target path does not exist (`--fix` still scaffolds it).
- `esmeril check` now reports each failing check's state - `missing`, `invalid` or `broken` - so a config that exists but is wrong is no longer described as "missing". The `project paths` check also shows the offending `$path` targets.
- `esmeril check --json` (with `--fix`) now includes the list of created files and configs that need manual attention in the report, via a `fix` field.
- `esmeril deps` / `esmeril update` now report dependencies in a stable order (section, then name), regardless of the order in `wally.toml`, and print per-package progress on stderr while auditing the registry.
- `esmeril init` now kebab-cases the project name for `wally.toml` (`esmeril init "My Game"` → `user/my-game`), so a scaffolded project always passes its own check; `--lib` output shows the publish hint.
- `esmeril check --markdown` now includes the info rows (README, .gitignore, tools on PATH).
- `esmeril fmt` fails with a clear message when `src/` is missing.

### Changed
- `esmeril doctor` falls back to stderr when a tool prints its version there (common on Windows).
- The scaffold now pins StyLua 2.5.2 and uses `actions/checkout@v7` in the generated CI workflow.
- Documented `esmeril check --markdown` and `esmeril completions` in the README.

## 0.1.1

### Changed
- Replaced unicode dashes in the docs with plain hyphens and restored the code comments that were missing.

## 0.1.0

### Added
- `esmeril init` - scaffold a Roblox project with Rojo, Selene, StyLua, Aftman, Wally and a GitHub Actions CI workflow preconfigured. `--strict` sets the Luau language mode to Strict, `--lib` scaffolds a library package (a single `src/init.lua` module, ready to publish to Wally) instead of a game, `--force` overwrites a non-empty target.
- `esmeril check` - inspect a project and print an A-F grade: validates `default.project.json` (including that every `$path` target exists), `.luaurc`, `.selene.toml`, `stylua.toml`, `aftman.toml` and `wally.toml`, plus `src/` and a CI workflow. Exit code 1 when the grade is D or F, or when `default.project.json` is missing/invalid or a `$path` target does not exist, so it is safe to gate CI on. `--json` emits a machine-readable report. `--fix` creates the standard files that are missing - never overwrites existing ones, and lists configs that exist but are invalid - then re-grades the project.
- `esmeril deps` - audit `wally.toml` dependencies against the package registry (GitHub-hosted indexes only). Reads `scope/name@version` specs plus the `[server-dependencies]` and `[dev-dependencies]` sections, resolves SemVer requirements (bare versions behave as `^`; `=` and `~` supported), and reports each dependency as ok, outdated, missing, not found, git or unsupported. Exit code 1 when any dependency is outdated, missing, not found or errored. `--json` emits a machine-readable report. Index manifests are cached locally for an hour (`$ESMERIL_CACHE` overrides the location); `--offline` uses only the cache.
- `esmeril update` - plan dependency bumps: every `wally.toml` requirement whose version range excludes a newer published release is shown as `old → new`. Dry-run by default; `--write` rewrites `wally.toml` in place, preserving comments and formatting. Exit code 1 when a dependency cannot be checked. Shares the index cache and supports `--offline`.
- `esmeril doctor` - check the local toolchain: rojo, selene, stylua, wally and aftman, with their versions and install hints when missing. Exit code 1 when any of the core tools (rojo, selene, stylua) is missing.
- `esmeril fmt` - run StyLua and Selene on `src/`: format by default, `--check` only reports. Exit code 1 on any problem or missing tool.
- `esmeril build` - run check first (refuses to build a failing project), then `rojo build`. Outputs `game.rbxl` or `lib.rbxm` by default, `-o` overrides.
- `esmeril check --markdown` - print the grade as a markdown table, ready to paste into a PR or README.
- `esmeril completions <shell>` - generate completion scripts for bash, zsh, fish, powershell or elvish.
