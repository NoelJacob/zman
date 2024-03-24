use clap::{value_parser, ColorChoice, Parser, Subcommand, ValueHint};
use directories::{BaseDirs, ProjectDirs};
use eyre::{ContextCompat, eyre, OptionExt, WrapErr};
use reqwest::blocking::get;
use serde_json::Value;
use std::env::consts::{ARCH, OS};
use std::path::PathBuf;
use std::process::exit;

#[derive(Parser)]
#[command(version, about, color = ColorChoice::Auto, help_expected = true, disable_help_subcommand = true, long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Download and set a Zig version as default.
    Default {
        #[arg(long, value_parser = value_parser!(PathBuf), value_hint = ValueHint::DirPath, value_name = "DIR")]
        /// Custom installation directory.
        install: Option<PathBuf>,
        #[arg(long, value_parser = value_parser!(PathBuf), value_hint = ValueHint::DirPath, value_name = "DIR")]
        /// Custom symlink directory.
        link: Option<PathBuf>,
        #[arg(long, conflicts_with = "link")]
        /// Create the symlink in user installation directory.
        user: bool,
        #[arg(long)]
        /// Do no create shims like zig-cc and zig-c++ for Zig drop-in replacements.
        no_dropins: bool,
        /// Exact version number or use latest for latest release or master for latest build.
        version: String,
    },
    /// Download a zig version.
    Fetch {
        #[arg(long, value_parser = value_parser!(PathBuf), value_hint = ValueHint::DirPath, value_name = "DIR")]
        /// Custom installation directory.
        install: Option<PathBuf>,
        /// Exact version number or use latest for latest release or master for latest build.
        version: String,
    },
    /// Clean everything except default and master, or give a specific version to clean just that.
    Clean {
        /// Exact version name or use latest for latest release and master for latest build.
        version: Option<String>,
    },
    /// List all installed versions.
    List,
    /// Prevent a version from being cleaned by zigup clean. Can be reverted by running clean with the particular version.
    Keep {
        /// Exact version name or use latest for latest release and master for latest build.
        version: String,
    },
    /// Run a specific version of Zig with the given arguments.
    Run {
        /// Exact version name or use latest for latest release and master for latest build.
        version: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, value_hint = ValueHint::CommandWithArguments)]
        /// Arguments to invoke Zig with.
        args: Vec<String>,
    },
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}



fn parse_ziglang_api(version: &str) -> (String, String) {
    let api = get("https://ziglang.org/download/index.json").expect("Cannot connect to ziglang.org API")
        .json::<Value>().expect("Cannot convert response to JSON");

    match version {
        "master" => {
            let master = api.get("master").expect("Cannot get master from API");
            let real_version = master
                .get("version").expect("Cannot get master.version from API")
                .to_string();
            let url = master
                .pointer(&*format!("{}-{}/tarball", ARCH, OS)).expect(&*format!(
                    "Cannot get master.{}-{}.tarball from API",
                    ARCH, OS
                ))
                .to_string();
            (real_version, url)
        }
        "latest" => {
            let (_, latest) = api
                .as_object().expect("Cannot parse API")
                .iter().next().expect("Cannot get 2nd element from API");
            let real_version = latest
                .get("version").expect("Cannot get latest.version from API")
                .to_string();
            let url = latest
                .get(format!("{}-{}/tarball", ARCH, OS)).expect(&*format!(
                    "Cannot get latest.{}-{}.tarball from API",
                    ARCH, OS
                ))
                .to_string();
            (real_version, url)
        }
        version => {
            let (_, latest) = api
                .as_object().expect("Cannot parse API")
                .iter()
                .find(|(key, _)| key.starts_with(version)).expect(&*format!("Cannot get {} from API", version));
            let real_version = latest
                .get("version").expect(&*format!("Cannot get {}.version from API", version))
                .to_string();
            let url = latest
                .get(format!("{}-{}/tarball", ARCH, OS)).expect(&*format!(
                    "Cannot get {}.{}-{}.tarball from API",
                    version, ARCH, OS
                ))
                .to_string();
            (real_version, url)
        }
    }
}

fn fetch_version_if_needed(version: &str, location: &PathBuf) -> eyre::Result<()> {
    let url = format!("https://ziglang.org/download/{}.tar.xz", version);
    let fname = location.join(version);

    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let default_install = ProjectDirs::from("com", "", "zigup").expect("Cannot select default project dir")
        .data_dir()
        .canonicalize().expect("Cannot canonicalize default data dir");
    // TODO: Elevation
    let default_link = PathBuf::from("/usr/bin");

    match cli.cmd {
        Cmd::Default {
            install,
            link,
            user,
            no_dropins,
            version,
        } => {
            let install_location = if let Some(x) = link {
                x
            } else if user {
                BaseDirs::new().expect("Cannot get $HOME")
                    .executable_dir().expect("Cannot get local bin home")
                    .canonicalize().expect("Cannot canonicalize local bin home")
            } else {
                default_link
            };

            let (real_version, url) = parse_ziglang_api(&version);
            let version_install_location = install_location.join(real_version);
            if version_install_location.try_exists().expect(&*format!("Cannot check if {:?} exists", version_install_location)) {
                //     TODO: copy zig to
            }
        }
        Cmd::Fetch { .. } => invalid(),
        Cmd::Clean { .. } => invalid(),
        Cmd::List => invalid(),
        Cmd::Keep { .. } => invalid(),
        Cmd::Run { .. } => invalid(),
    }

    // Ok(())
}
