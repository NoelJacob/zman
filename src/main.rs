mod download;

use clap::{value_parser, ColorChoice, Parser, Subcommand, ValueHint};
use console::Term;
use directories::{BaseDirs, ProjectDirs};
use download::download_file;
use eyre::{bail, ensure, eyre, OptionExt, Result, WrapErr};
use fs_extra::dir::{copy, CopyOptions};
use reqwest::Client;
use semver::{Version, VersionReq};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::env::consts::{ARCH, OS};
use std::env::var;
use std::fs::{create_dir_all, read_dir, remove_file, set_permissions, write, File, Permissions};
use std::io::{ErrorKind, Read};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use tar::Archive;
use temp_dir::TempDir;
use tokio::runtime::Runtime;
use xz2::read::XzDecoder;

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
fn it_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}

#[test]
fn it_parse_ziglang_api() {
    // let x = parse_ziglang_api("0.10").unwrap();
    // dbg!(x);
}

#[test]
fn it_sudo() {
    let x = ProjectDirs::from("com", "", "zman")
        .ok_or_eyre("Default project directory could not be selected")
        .unwrap();
    let install_default = x.data_dir();
    let x = BaseDirs::new()
        .ok_or_eyre("User home directory could not be found")
        .unwrap();
    let link_default = x
        .executable_dir()
        .ok_or_eyre("Local bin directory could not be found")
        .unwrap();

    println!(
        "{} {}",
        install_default.to_str().unwrap(),
        link_default.to_str().unwrap()
    );
}

#[test]
fn it_download() {
    // let x = download_tarxz("https://ziglang.org/download/0.11.0/zig-linux-x86_64-0.11.0.tar.xz");
    // dbg!(r);
}

#[test]
fn it_sha256() {
    check_sha256(
        &PathBuf::from("/tmp/zig-linux-x86_64-0.11.0.tar.xz"),
        "2d00e789fec4f71790a6e7bf83ff91d564943c5ee843c5fd966efc474b423047".to_string(),
    )
    .unwrap();
}

#[test]
fn it_extract() {
    // extract_tarxz(
    //     &PathBuf::from("/tmp/zig-linux-x86_64-0.11.0.tar.xz"),
    //     &PathBuf::from("/tmp/zig")
    // )
    // .unwrap();
}

#[test]
fn it_symlink() {
    make_symlink(
        &PathBuf::from("/tmp/zig/0.11.0"),
        &PathBuf::from("/tmp/bin"),
        false,
    )
    .unwrap();
}

async fn parse_ziglang_api(client: &Client, version: &str) -> Result<(String, String, String)> {
    let _e = "API could not be parsed";

    let tarball = format!("/{}-{}/tarball", ARCH, OS);
    let sha = format!("/{}-{}/shasum", ARCH, OS);
    let _e_arch = || eyre!("Zig binary for {} target not available", tarball);

    let api = client
        .get("https://ziglang.org/download/index.json")
        .send()
        .await
        .wrap_err_with(|| "Cannot connect to ziglang.org API")?
        .json::<Value>()
        .await
        .wrap_err_with(|| _e)?;

    match version {
        "master" => {
            let specific_version = api
                .pointer("/master/version")
                .ok_or_eyre(_e)?
                .as_str()
                .ok_or_eyre(_e)?
                .to_string();
            let url = api
                .pointer(&format!("/master{}", tarball))
                .ok_or_else(_e_arch)?
                .as_str()
                .ok_or_eyre(_e)?
                .to_string();
            let shasum = api
                .pointer(&format!("/master{}", sha))
                .ok_or_else(_e_arch)?
                .as_str()
                .ok_or_eyre(_e)?
                .to_string();
            Ok((specific_version, url, shasum))
        }
        "latest" => {
            let api_map = api.as_object().ok_or_eyre(_e)?;
            let mut latest: Option<&Value> = None;
            let mut latest_date: Option<&str> = None;
            let mut latest_version: Option<&str> = None;
            for (ver, val) in api_map {
                if ver != "master" {
                    match latest_date {
                        None => {
                            let date = val
                                .pointer("/date")
                                .ok_or_eyre(_e)?
                                .as_str()
                                .ok_or_eyre(_e)?;
                            latest = Some(val);
                            latest_date = Some(date);
                            latest_version = Some(ver);
                        }
                        Some(x) => {
                            let val_date = val
                                .pointer("/date")
                                .ok_or_eyre(_e)?
                                .as_str()
                                .ok_or_eyre(_e)?;
                            if x < val_date {
                                latest = Some(val);
                                latest_date = Some(val_date);
                                latest_version = Some(ver);
                            }
                        }
                    }
                }
            }
            let specific_version = latest_version
                .ok_or_eyre("Latest version could not be found")?
                .to_string();
            let url = latest
                .ok_or_eyre(_e)?
                .pointer(&tarball)
                .ok_or_else(_e_arch)?
                .as_str()
                .ok_or_eyre(_e)?
                .to_string();
            let shasum = latest
                .ok_or_eyre(_e)?
                .pointer(&sha)
                .ok_or_else(_e_arch)?
                .as_str()
                .ok_or_eyre(_e)?
                .to_string();
            Ok((specific_version, url, shasum))
        }
        version => {
            let api_map = api.as_object().ok_or_eyre(_e)?;
            let version_required = VersionReq::parse(&format!("={}", version))?;
            let latest_matching_version = api_map
                .keys()
                .filter_map(|x| Version::parse(x).ok())
                .filter(|x| version_required.matches(x))
                .max();
            let specific_version = latest_matching_version
                .ok_or_else(|| eyre!("Version {} could not be found", version))?
                .to_string();
            let url = api
                .pointer(&format!("/{}{}", specific_version, tarball))
                .ok_or_else(_e_arch)?
                .as_str()
                .ok_or_eyre(_e)?
                .to_string();
            let shasum = api
                .pointer(&format!("/{}{}", specific_version, sha))
                .ok_or_else(_e_arch)?
                .as_str()
                .ok_or_eyre(_e)?
                .to_string();
            Ok((specific_version, url, shasum))
        }
    }
}

fn add_dropins(destination: &Path, dropins: [&str; 8]) -> Result<()> {
    for x in dropins {
        let file = format!("#!/bin/bash\nexec zig {} \"$@\"", x);
        let path = destination.join("zig-".to_string() + x);
        write(&path, file)?;
        set_permissions(&path, Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn rm_dropins(destination: &Path, dropins: [&str; 8]) -> Result<()> {
    for x in dropins {
        let path = destination.join("zig-".to_string() + x);
        match remove_file(&path) {
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => bail!(e),
            Ok(_) => {}
        };
    }
    Ok(())
}

fn make_symlink(source: &Path, destination: &Path, no_dropins: bool) -> Result<()> {
    let dropins = [
        "ar", "cc", "c++", "dlltool", "lib", "ranlib", "objcopy", "rc",
    ];
    create_dir_all(destination)?;
    match symlink(source.join("zig"), destination.join("zig")) {
        Ok(_) => {
            print!("Zig added at {:?}", destination);
            if !no_dropins {
                add_dropins(destination, dropins)?;
                println!(" with drop-in tools");
            }
            if !var("PATH")?.contains(destination.to_str().ok_or_eyre("Path Invalid")?) {
                println!("Add it to PATH");
            };
        }
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            bail!("Permission denied to create symlink at {:?}. Do NOT RUN as root. Try passing a custom symlink directory with --link option", destination)
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            remove_file(destination.join("zig"))?;
            rm_dropins(destination, dropins)?;
            make_symlink(source, destination, no_dropins)?;
        }
        Err(e) => bail!(e),
    };
    Ok(())
}

fn check_sha256(file: &PathBuf, hash: String) -> Result<()> {
    let mut file = File::open(file)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 4096];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    let x = format!("{:x}", hasher.finalize());
    ensure!(hash == x);
    Ok(())
}

fn extract_and_copy(
    file: &Path,
    extract_location: PathBuf,
    install_location: &PathBuf,
) -> Result<()> {
    let _e = || eyre!("Extracting {:?} failed", file);
    let xz = XzDecoder::new(File::open(file).wrap_err_with(_e)?);
    let mut tar = Archive::new(xz);
    let t = Term::stdout();
    t.write_line("Extracting Zig...")?;
    tar.unpack(&extract_location)?;
    t.clear_line()?;
    t.write_line("Installing Zig...")?;
    create_dir_all(install_location)?;
    let mut opts = CopyOptions::new();
    opts.overwrite = true;
    opts.content_only = true;
    let inside_dir = read_dir(extract_location)?
        .next()
        .ok_or_eyre("Extracted directory not found")??;
    create_dir_all(install_location)?;
    copy(inside_dir.path(), install_location, &opts)?;
    t.clear_line()?;
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let x = ProjectDirs::from("com", "", "zman")
        .ok_or_eyre("Default project directory could not be selected")?;
    let install_default = x.data_dir();
    let x = BaseDirs::new().ok_or_eyre("User home directory could not be found")?;
    let link_default = x
        .executable_dir()
        .ok_or_eyre("Local bin directory could not be found")?;

    match cli.cmd {
        Cmd::Default {
            install,
            link,
            no_dropins,
            version,
        } => {
            // link_location: ./local/bin/ -symlink-> version_link_location
            // install_location: ./local/share/zman/
            // version_install_location: ./local/share/zman/0.11.0/ or ./local/share/zman/master/
            // Not Implemented - version_link_location: ./local/share/zman/bin/

            let link_location = match &link {
                Some(x) => x,
                None => link_default,
            };
            let install_location = match &install {
                Some(x) => x,
                None => install_default,
            };

            let client = Client::new();
            let rt = Runtime::new()?;

            let (specific_version, url, shasum) =
                rt.block_on(parse_ziglang_api(&client, &version))?;
            let specific_install_location = if "master" == &version {
                install_location.join("master")
            } else {
                install_location.join(&specific_version)
            };
            if version != "master"
                && specific_install_location
                    .join("zig")
                    .try_exists()
                    .wrap_err_with(|| {
                        eyre!("Cannot check if {:?} already downloaded", specific_version)
                    })?
            {
                println!("Zig version {} already downloaded", specific_version);
            } else {
                let temp = TempDir::with_prefix("zman")?;
                let extract_location = temp.child(&version);
                let tarxz = temp.child(format!("zig-{}.tar.xz", version));
                rt.block_on(download_file(&client, &url, &tarxz))
                    .wrap_err_with(|| eyre!("Downloading {:?} failed", specific_version))?;
                check_sha256(&tarxz, shasum)
                    .wrap_err_with(|| eyre!("Checksum failed for {:?}", specific_version))?;
                extract_and_copy(&tarxz, extract_location, &specific_install_location)?;
            }
            make_symlink(&specific_install_location, link_location, no_dropins)?;
        }
        Cmd::Fetch { .. } => bail!("Not implemented"),
        Cmd::Clean { .. } => bail!("Not implemented"),
        Cmd::List => bail!("Not implemented"),
        Cmd::Keep { .. } => bail!("Not implemented"),
        Cmd::Run { .. } => bail!("Not implemented"),
    }

    Ok(())
}
