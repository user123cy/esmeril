use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;

pub fn files(name: &str, strict: bool, lib: bool) -> BTreeMap<String, String> {
    let mode = if strict { "Strict" } else { "NonStrict" };
    let mut out = BTreeMap::new();
    out.insert("default.project.json".into(), project_json(name, lib));
    out.insert(".luaurc".into(), luaurc(mode));
    out.insert(".selene.toml".into(), SELENE.to_string());
    out.insert("stylua.toml".into(), STYLUA.to_string());
    out.insert("aftman.toml".into(), AFTMAN.to_string());
    out.insert("wally.toml".into(), wally_toml(name));
    out.insert(".gitignore".into(), GITIGNORE.to_string());
    out.insert("README.md".into(), readme(name, lib));
    out.insert(".github/workflows/ci.yml".into(), ci(lib));
    if lib {
        out.insert("src/init.lua".into(), LIB_ENTRY.to_string());
    } else {
        out.insert("src/shared/init.lua".into(), SHARED.to_string());
        out.insert("src/server/init.server.lua".into(), SERVER.to_string());
        out.insert("src/client/init.client.lua".into(), CLIENT.to_string());
    }
    out
}

pub fn write_all(root: &Path, files: &BTreeMap<String, String>) -> anyhow::Result<()> {
    for (rel, content) in files {
        write_one(root, rel, content)?;
    }
    Ok(())
}

pub fn write_one(root: &Path, rel: &str, content: &str) -> anyhow::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }
    std::fs::write(&path, content).with_context(|| format!("failed to write '{}'", path.display()))
}

fn project_json(name: &str, lib: bool) -> String {
    let tree = if lib {
        serde_json::json!({ "$path": "src" })
    } else {
        serde_json::json!({
            "$className": "DataModel",
            "ReplicatedStorage": {
                "$className": "ReplicatedStorage",
                "shared": { "$path": "src/shared" }
            },
            "ServerScriptService": {
                "$className": "ServerScriptService",
                "server": { "$path": "src/server" }
            },
            "StarterPlayer": {
                "$className": "StarterPlayer",
                "StarterPlayerScripts": {
                    "$className": "StarterPlayerScripts",
                    "client": { "$path": "src/client" }
                }
            }
        })
    };
    serde_json::to_string_pretty(&serde_json::json!({ "name": name, "tree": tree }))
        .expect("serializing a project tree cannot fail")
}

fn luaurc(mode: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "languageMode": { "src/": mode }
    }))
    .expect("serializing .luaurc cannot fail")
}

fn wally_toml(name: &str) -> String {
    format!(
        "[package]\nname = \"user/{name}\"\nversion = \"0.1.0\"\nregistry = \"https://github.com/UpliftGames/wally-index\"\nrealm = \"shared\"\n\n[dependencies]\n"
    )
}

fn readme(name: &str, lib: bool) -> String {
    if lib {
        format!(
            "# {name}\n\nRoblox Luau library scaffolded with esmeril.\n\n## setup\n\n1. install [aftman](https://github.com/rojo-rbx/aftman)\n2. `aftman install` - downloads Rojo, Selene and StyLua\n\n## publish\n\n1. set the real `scope` in `wally.toml` (`name = \"scope/{name}\"`)\n2. `wally login`\n3. `wally publish`\n\n## checks\n\n```\nesmeril check\n```\n"
        )
    } else {
        format!(
            "# {name}\n\nRoblox game scaffolded with esmeril.\n\n## setup\n\n1. install [aftman](https://github.com/rojo-rbx/aftman)\n2. `aftman install` - downloads Rojo, Selene and StyLua\n3. install the [Rojo](https://rojo.space) plugin in Roblox Studio\n4. `rojo serve`, then connect from Studio\n\n## structure\n\n- `src/server` - ServerScriptService\n- `src/client` - StarterPlayerScripts\n- `src/shared` - ReplicatedStorage modules\n\n## checks\n\n```\nesmeril check\n```\n"
        )
    }
}

const SELENE: &str = "std = \"roblox\"\n";

const STYLUA: &str =
    "indent_type = \"Tabs\"\nindent_width = 4\nquote_style = \"AutoPreferSingle\"\n";

const AFTMAN: &str = "[tools]\nrojo = \"rojo-rbx/rojo@7.7.0\"\nselene = \"Kampfkarren/selene@0.31.0\"\nstylua = \"JohnnyMorganz/StyLua@2.5.0\"\n";

const GITIGNORE: &str = "target/\n.lune/\n*.rbxl\n*.rbxlx\n*.rbxmx\n*.rbxm\nThumbs.db\n.DS_Store\n";

fn ci(lib: bool) -> String {
    let build = if lib {
        "      - run: rojo build -o lib.rbxm\n"
    } else {
        "      - run: rojo build -o game.rbxl\n"
    };
    format!(
        "name: ci\n\non:\n  push:\n  pull_request:\n\njobs:\n  luau:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: Roblox/setup-aftman-action@v2\n        with:\n          token: ${{{{ secrets.GITHUB_TOKEN }}}}\n      - run: aftman install\n      - run: selene src\n      - run: stylua --check src\n{build}"
    )
}

const SHARED: &str = "-- shared modules: require with ReplicatedStorage.shared\n";

const SERVER: &str = "-- server entry point (ServerScriptService)\nprint(\"server started\")\n";

const CLIENT: &str = "-- client entry point (StarterPlayerScripts)\nprint(\"client started\")\n";

const LIB_ENTRY: &str = "-- module entry point; requires return this table\nreturn {}\n";
