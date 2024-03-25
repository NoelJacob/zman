use clap::{value_parser, ColorChoice, Parser, Subcommand, ValueHint};
use directories::{BaseDirs, ProjectDirs};
use eyre::{OptionExt, WrapErr, Result, eyre, bail, ContextCompat};
use reqwest::blocking::get;
use serde_json::Value;
use std::env::consts::{ARCH, OS};
use std::path::PathBuf;
use std::process::exit;
use semver::{Version, VersionReq};

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
    /// Prevent a version from being cleaned by clean command. Can be reverted by running clean with the particular version.
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

#[test]
fn it_parses_ziglang_api() {
    // let x = parse_ziglang_api("0.11").unwrap();
    dbg!(VersionReq::parse("0.11").unwrap().matches(&Version::parse("0.11.100").unwrap()));
    // dbg!(x);
}

fn parse_ziglang_api(version: &str) -> Result<(String, String)> {
    let _e = "API could not be parsed";

    let arch = format!("/{}-{}/tarball", ARCH, OS);
    let _e_arch = || eyre!("Zig binary for {} target not available", arch);

    let api = get("https://ziglang.org/download/index.json").wrap_err_with(|| "Cannot connect to ziglang.org API")?
        .json::<Value>().wrap_err_with(|| _e)?;

    match version {
        "master" => {
            let real_version = api
                .pointer("/master/version").ok_or_eyre(_e)?
                .as_str().ok_or_eyre(_e)?.to_string();
            let url = api
                .pointer(&format!("/master{}",arch)).ok_or_else(_e_arch)?
                .as_str().ok_or_eyre(_e)?.to_string();
            Ok((real_version, url))
        }
        "latest" => {
            let api_map = api
                .as_object().ok_or_eyre(_e)?;
            // TODO: Compare dates in for loop
            let mut latest: Option<&Value> = None;
            let mut latest_date: Option<&str> = None;
            let mut latest_version: Option<&str> = None;
            for (ver, val) in api_map {
                if ver != "master" {
                    match latest_date {
                        None => {
                            let date = val.pointer("/date").ok_or_eyre(_e)?
                                .as_str().ok_or_eyre(_e)?;
                            latest = Some(val);
                            latest_date = Some(date);
                            latest_version = Some(ver);
                        }
                        Some(x) => {
                            let val_date = val.pointer("/date").ok_or_eyre(_e)?
                                .as_str().ok_or_eyre(_e)?;
                            if x < val_date {
                                latest = Some(val);
                                latest_date = Some(val_date);
                                latest_version = Some(ver);
                            }
                        }
                    }
            }
            }
            let real_version = latest_version.ok_or_eyre(_e)?
                .to_string();
            let url = latest.ok_or_eyre(_e)?
                .pointer(&arch).ok_or_else(_e_arch)?
                .as_str().ok_or_eyre(_e)?.to_string();
            Ok((real_version, url))
        }
        version => {
            let api_map= api
                .as_object().ok_or_eyre(_e)?;
            let mut latest_matching_version_list: Option<Vec<Version>> = None;
            let sem_version = VersionReq::parse(&format!("={}",version))?;
            // api_map.remove()
            for (ver, _) in api_map {
                if ver != "master" {
                    match latest_matching_version_list.take() {
                        None => {
                            let sem_ver = Version::parse(ver)?;
                            latest_matching_version_list = Some([sem_ver].to_vec());
                        }
                        Some(mut x) => {
                            let sem_ver = Version::parse(ver)?;
                            if sem_version.matches(&sem_ver) {
                                x.push(sem_ver);
                                latest_matching_version_list = Some(x);
                            }
                        }
                    }
                }
            }
            dbg!(&latest_matching_version_list);
            let real_version = latest_matching_version_list.ok_or_eyre(_e)?
                .iter().max()
                .ok_or_else(|| eyre!("Version {} not found", version))?
                .to_string();
            let url = api
                .pointer(&format!("/{}{}", real_version, arch)).ok_or_else(_e_arch)?
                .as_str().ok_or_eyre(_e)?.to_string();
            Ok((real_version, url))
        }
    }
}

// fn make_shims()

fn main() -> Result<()> {
    let cli = Cli::parse();

    let default_install = ProjectDirs::from("com", "", "zman").ok_or_eyre("Default project dir could not be selected")?
        .data_dir()
        .canonicalize()?;
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
            let install_location =  match link {
                None if user => BaseDirs::new().ok_or_eyre("$HOME could not be found")?
                    .executable_dir().ok_or_eyre("Local bin dir could not be found")?
                    .canonicalize()?,
                None => default_install,
                Some(x) => x,
            };

            let (real_version, url) = parse_ziglang_api(&version)?;
            let version_install_location = install_location.join(real_version);
            if version_install_location.try_exists().wrap_err_with(|| eyre!("Cannot check if {:?} exists", version_install_location))? {
                // TODO: make shims

            } else {
                // TODO: download zig
            }
        }
        Cmd::Fetch { .. } => bail!("Not implemented"),
        Cmd::Clean { .. } => bail!("Not implemented"),
        Cmd::List => bail!("Not implemented"),
        Cmd::Keep { .. } => bail!("Not implemented"),
        Cmd::Run { .. } => bail!("Not implemented"),
    }

    Ok(())
}
