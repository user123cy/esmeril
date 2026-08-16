use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "esmeril",
    version,
    about = "Scaffold, check and inspect Roblox Luau projects"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(
        global = true,
        long,
        id = "json_out",
        help = "Emit machine-readable JSON to stdout instead of the table"
    )]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(
        about = "Scaffold a modern Roblox project: Rojo, Selene, StyLua, Aftman, Wally and CI"
    )]
    Init(InitArgs),
    #[command(about = "Inspect a project: tooling configs, structure and an A-F grade")]
    Check(CheckArgs),
    #[command(about = "Audit wally.toml dependencies against the package registry")]
    Deps(DepsArgs),
    #[command(about = "Bump wally.toml requirements to the latest published versions")]
    Update(UpdateArgs),
    #[command(about = "Run the formatter and linter on the source tree")]
    Fmt(FmtArgs),
    #[command(about = "Check the project and build it with Rojo")]
    Build(BuildArgs),
    #[command(about = "Check the local toolchain: rojo, selene, stylua, wally and aftman")]
    Doctor,
    #[command(about = "Generate shell completion scripts")]
    Completions {
        #[arg(
            value_enum,
            help = "Shell to generate for: bash, zsh, fish, powershell, elvish"
        )]
        shell: clap_complete::Shell,
    },
}

#[derive(clap::Args)]
pub struct InitArgs {
    #[arg(help = "Directory to create; defaults to the current directory name")]
    pub name: Option<String>,

    #[arg(long, help = "Use Luau language mode Strict instead of NonStrict")]
    pub strict: bool,

    #[arg(
        long,
        help = "Scaffold a library package (for publishing to Wally) instead of a game"
    )]
    pub lib: bool,

    #[arg(long, help = "Overwrite files when the target directory is not empty")]
    pub force: bool,
}

#[derive(clap::Args)]
pub struct CheckArgs {
    #[arg(
        default_value = ".",
        help = "Project directory to inspect; defaults to the current directory"
    )]
    pub path: String,

    #[arg(
        long,
        help = "Create the standard files that are missing instead of only reporting"
    )]
    pub fix: bool,

    #[arg(
        long,
        help = "Print the report as a markdown table instead of the text report"
    )]
    pub markdown: bool,
}

#[derive(clap::Args)]
pub struct DepsArgs {
    #[arg(
        default_value = ".",
        help = "Project directory to inspect; defaults to the current directory"
    )]
    pub path: String,

    #[arg(
        long,
        help = "Use only the local index cache; fail when a package is not cached"
    )]
    pub offline: bool,
}

#[derive(clap::Args)]
pub struct FmtArgs {
    #[arg(
        default_value = ".",
        help = "Project directory to inspect; defaults to the current directory"
    )]
    pub path: String,

    #[arg(long, help = "Only report problems, do not format")]
    pub check: bool,
}

#[derive(clap::Args)]
pub struct BuildArgs {
    #[arg(
        default_value = ".",
        help = "Project directory to inspect; defaults to the current directory"
    )]
    pub path: String,

    #[arg(
        short,
        long,
        help = "Output file name (defaults to game.rbxl or lib.rbxm)"
    )]
    pub output: Option<String>,
}

#[derive(clap::Args)]
pub struct UpdateArgs {
    #[arg(
        default_value = ".",
        help = "Project directory to inspect; defaults to the current directory"
    )]
    pub path: String,

    #[arg(
        long,
        help = "Write the new requirements to wally.toml (dry-run by default)"
    )]
    pub write: bool,

    #[arg(
        long,
        help = "Use only the local index cache; fail when a package is not cached"
    )]
    pub offline: bool,
}
