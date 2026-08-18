# esmeril

![crates.io](https://img.shields.io/crates/v/esmeril.svg)
![downloads](https://img.shields.io/crates/d/esmeril.svg)
![License](https://img.shields.io/badge/license-MIT-blue)
![CI](https://github.com/user123cy/esmeril/actions/workflows/ci.yml/badge.svg)

Scaffold, validate, format, audit and build Roblox Luau projects from the terminal.

Eight commands, one binary:

- `esmeril init` - scaffold a modern Roblox project: Rojo, Selene, StyLua, Aftman, Wally and a CI workflow, preconfigured
- `esmeril check` - verify the tooling configs and structure, get an A-F grade; exit code 1 on a failing grade, safe to gate CI on
- `esmeril deps` - audit `wally.toml` against the registry: outdated, missing and unknown packages; exit code 1 on any problem
- `esmeril doctor` - check the local toolchain: rojo, selene, stylua, wally and aftman, with versions and install hints
- `esmeril update` - bump `wally.toml` requirements to the latest published versions; `--write` applies, dry-run by default
- `esmeril fmt` - format with StyLua and lint with Selene; `--check` only reports
- `esmeril build` - run check, then build with Rojo (`game.rbxl` or `lib.rbxm`)
- `esmeril completions <shell>` - generate shell completion scripts (bash, zsh, fish, powershell, elvish)

## install

From crates.io:

```
cargo install esmeril
```

Or download a prebuilt binary for Linux, macOS or Windows from the [releases](https://github.com/user123cy/esmeril/releases) page.

## scaffold

```
esmeril init mygame
cd mygame
aftman install   # downloads Rojo, Selene and StyLua
rojo serve       # connect from Studio
```

`esmeril init` writes the files a professional Roblox workflow needs, all wired together:

- `default.project.json` - Rojo project tree (`src/server`, `src/client`, `src/shared`)
- `.luaurc` - Luau language mode for the source tree
- `.selene.toml` + `stylua.toml` - lint and format config
- `aftman.toml` - pinned Rojo / Selene / StyLua versions
- `wally.toml` - package manifest
- `.github/workflows/ci.yml` - lint + format + build on every push

Flags: `--strict` sets the Luau language mode to Strict instead of NonStrict, `--lib` scaffolds a library package (a single `src/init.lua` module, ready to publish to Wally) instead of a game, `--force` overwrites a non-empty target directory. The directory name is kebab-cased for the `wally.toml` package name (`esmeril init "My Game"` → `user/my-game`), so a scaffolded project always passes its own check.

## check

```
esmeril check
esmeril check path/to/project
esmeril check --fix
esmeril check --json
esmeril check --markdown
```

`esmeril check` validates the project and grades it A-F. `--fix` creates the standard files that are missing instead of only reporting them - point it at any messy project and watch the grade jump from F to A. It never overwrites an existing file; a config that exists but is invalid is listed as needing manual attention.

| check | weight | fails when |
|---|---|---|
| `default.project.json` | 15 | missing, invalid JSON, or no `name` |
| project paths | 10 | a `$path` target in the tree does not exist |
| `src/` | 10 | missing |
| `.luaurc` | 10 | missing, invalid JSON, or no valid `languageMode` (`NonStrict`/`Strict`) |
| `.selene.toml` | 15 | missing or invalid TOML |
| `stylua.toml` | 15 | missing or invalid TOML |
| `aftman.toml` | 15 | missing, invalid TOML, or no rojo + selene + stylua as `owner/repo@tag` under `[tools]` |
| `wally.toml` | 5 | missing, invalid TOML, or no complete `[package]` (scoped name, version, registry, realm) |
| `.github/workflows` | 5 | no workflow file |

Missing `wally.toml` or the CI workflow are recommended, not required - a project without both still grades A (90/100). `README.md`, `.gitignore` and whether `rojo`/`selene`/`stylua` are on `PATH` are shown as info, not scored. `--markdown` prints the report as a markdown table, ready to paste into a PR or README.

## deps

```
esmeril deps
esmeril deps path/to/project
esmeril deps --offline
esmeril deps --json
```

`esmeril deps` reads `wally.toml` and checks every dependency against the package registry (GitHub-hosted indexes only, like the official `https://github.com/UpliftGames/wally-index`). Dependencies are written in the Wally form `scope/name@version`; git dependencies and the `[server-dependencies]`/`[dev-dependencies]` sections are reported but not scored. Index manifests are cached locally for an hour (`$ESMERIL_CACHE` overrides the location) and reused on later runs; `--offline` uses only the cache and fails on packages that are not cached.

Each dependency gets a status:

| status | meaning |
|---|---|
| ok | the newest published version satisfies your requirement |
| outdated | a newer version exists but your requirement excludes it (exit 1) |
| missing | no published version satisfies your requirement (exit 1) |
| not found | package does not exist in the registry (exit 1) |
| error | the registry could not be reached (exit 1) |
| git / unsupported | reported as info, not scored |

Versions follow SemVer: a bare requirement like `1.8.4` behaves as `^1.8.4` (Cargo style); `=1.8.4` pins exactly and `~1.8.4` allows patch updates only. `latest` is the newest stable release - pre-releases like `1.13.0-rc.3` are never considered a target.

## update

```
esmeril update
esmeril update path/to/project --write
```

`esmeril update` finds dependencies whose requirement excludes a newer published version and proposes the bump, one line per dependency. It is a dry-run by default - `--write` rewrites `wally.toml` in place, preserving comments and formatting. A dependency pinned with `=1.8.4` is left alone; a caret requirement like `1.8.4` is bumped to the latest stable, matching what `wally update` would resolve. Like `deps`, it uses the local index cache and supports `--offline`.

## fmt

```
esmeril fmt
esmeril fmt --check
esmeril fmt path/to/project
```

`esmeril fmt` runs StyLua and Selene on `src/`: format and fix by default, `--check` only reports problems (exit code 1 on any). Missing tools show the install command instead of failing silently.

## build

```
esmeril build
esmeril build path/to/project -o build.rbxl
```

`esmeril build` runs the check first and refuses to build a failing project, then calls `rojo build`. Output defaults to `game.rbxl` for games and `lib.rbxm` for libraries; `-o` overrides.

## doctor

```
esmeril doctor
```

`esmeril doctor` checks that the local toolchain is ready: rojo, selene, stylua, wally and aftman. Tools that are installed report their version (`tool --version`); missing ones show the install command. Exit code 1 when any of the core tools (rojo, selene, stylua) is missing - run it on a fresh machine to see exactly what is left to install.

## exit codes

`esmeril check` exits 0 when the grade is A, B or C, and 1 when it is D or F - including when a `$path` in `default.project.json` points at a directory that does not exist. `esmeril deps` exits 1 when any dependency is outdated, missing, not found or errored. `esmeril update` exits 1 when a dependency cannot be checked. `esmeril fmt` exits 1 when a tool is missing or a check fails. `esmeril build` exits 1 when check or the Rojo build fails. `esmeril doctor` exits 1 when a core tool is missing. `esmeril check --fix` exits with the grade after fixing. That makes them safe to gate CI on. On Windows PowerShell use `$LASTEXITCODE` to read it - `$?` is a boolean there and prints `False` for a non-zero exit.

## why

Setting up a professional Roblox workflow means installing and configuring seven separate tools - Rojo, Selene, StyLua, Aftman, Wally, a language mode and CI - by hand, each with its own docs. esmeril is one binary and a few flags: `esmeril init` produces the whole setup, `esmeril check` keeps it honest (and `--fix` repairs it), `esmeril fmt` formats and lints, `esmeril build` compiles it, `esmeril deps` keeps dependencies current, `esmeril update` bumps them and `esmeril doctor` verifies the machine.

## license

MIT
