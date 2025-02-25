use std::{env, ffi::OsString, fs::read_dir, path::PathBuf};

use chrono::DateTime;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use xshell::{cmd, Shell};

const MUSL_DIR: &str = "x86_64-unknown-linux-musl";

#[derive(Subcommand, Clone, Debug)]
pub enum CliCommand {
    Dist,

    /// fmt taplo, rust and clippy
    Fmt,

    /// Create pre-commit hooks with gitleaks
    SetupRepo,
}

#[derive(Clone, Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

fn get_project_root() -> Result<PathBuf> {
    let path = env::current_dir()?;
    let path_ancestors = path.as_path().ancestors();

    for p in path_ancestors {
        let has_cargo = read_dir(p)?.any(|p| p.unwrap().file_name() == *"Cargo.lock");
        if has_cargo {
            return Ok(PathBuf::from(p));
        }
    }
    Err(anyhow!("Could not find Cargo.lock."))
}

#[derive(Debug, serde::Deserialize)]
pub struct Package {
    pub version: String,
    pub name: String,
    pub license: String,
    pub description: String,
}

fn read_package(package: &str) -> Result<Package> {
    let root = get_project_root()?;
    let cargo_toml = std::fs::read_to_string(root.join(package).join("Cargo.toml"))?;
    let json_content: serde_json::Value = toml::from_str(&cargo_toml)?;

    let pkg_info = json_content
        .get("package")
        .ok_or(anyhow!("Could not find package key in the Cargo.toml"))?;

    Ok(serde_json::from_value(pkg_info.clone())?)
}

pub fn binary_location(pkg: &Package) -> Result<std::path::PathBuf> {
    Ok(get_project_root()?
        .join("target")
        .join(MUSL_DIR)
        .join("release")
        .join(&pkg.name))
}

pub fn common_rpm(pkg: &Package) -> Result<rpm::PackageBuilder> {
    let sh = Shell::new()?;
    let last_commit_date = DateTime::from_timestamp(
        cmd!(sh, "git show --no-patch --format=%ct HEAD")
            .read()?
            .parse()?,
        0,
    )
    .ok_or(anyhow!("Could not use detected timestamp"))?;

    use std::os::unix::ffi::OsStringExt;
    let buildhost = OsString::from_vec(rustix::system::uname().nodename().to_bytes().to_vec());

    Ok(rpm::PackageBuilder::new(
        &pkg.name,
        &pkg.version,
        &pkg.license,
        std::env::consts::ARCH,
        &pkg.description,
    )
    .source_date(last_commit_date)
    .vendor("dzordzu")
    .build_host(
        buildhost
            .to_str()
            .ok_or(anyhow!("Could not detect hostname"))?,
    )
    .compression(rpm::CompressionType::Gzip)
    .url("https://github.com/dzordzu/o11y-389ds-rs"))
}

fn common_rpm_build(pkg: rpm::PackageBuilder, rpm_name: &str) -> Result<()> {
    let name = format!("{}.{}.rpm", rpm_name, std::env::consts::ARCH);
    let result_path = get_project_root()?.join("target").join("dist").join(name);

    std::fs::create_dir_all(result_path.parent().unwrap())?;

    pkg.build()?.write_file(result_path)?;

    Ok(())
}

fn nagios_389ds_rpm() -> Result<()> {
    let nagios_pkg = read_package("nagios-389ds-rs")?;
    let binary_location = binary_location(&nagios_pkg)?;

    let pkg = common_rpm(&nagios_pkg)?.with_file(
        binary_location,
        rpm::FileOptions::new("/usr/lib64/nagios/plugins/check_389ds_rs")
            .mode(rpm::FileMode::regular(0o755)),
    )?;

    common_rpm_build(pkg, &nagios_pkg.name)?;

    Ok(())
}

fn exporter_389ds_rpn() -> Result<()> {
    let exporter_pkg = read_package("exporter-389ds-rs")?;
    let binary_location = binary_location(&exporter_pkg)?;
    let root_dir = get_project_root()?;

    let create_user = "useradd -r exporter-389ds-rs; exit 0";
    let daemon_reload = "systemctl daemon-reload";
    let disable_unit = "systemctl disable exporter-389ds-rs.service";
    let delete_user_and_disable = "systemctl daemon-reload; userdel exporter-389ds-rs;";

    let pkg = common_rpm(&exporter_pkg)?
        .pre_install_script(create_user)
        .post_uninstall_script(delete_user_and_disable)
        .post_install_script(daemon_reload)
        .pre_uninstall_script(disable_unit)
        .with_file(
            binary_location,
            rpm::FileOptions::new("/usr/bin/exporter-389ds-rs").mode(rpm::FileMode::regular(0o755)),
        )?
        .with_file(
            root_dir.join("exporter-389ds-rs.service"),
            rpm::FileOptions::new("/etc/systemd/system/exporter-389ds-rs.service")
                .mode(rpm::FileMode::regular(0o644)),
        )?
        .with_file(
            root_dir.join("exporter-389ds-rs.minimal.toml"),
            rpm::FileOptions::new("/etc/o11y-389ds-rs/default.toml")
                .mode(rpm::FileMode::regular(0o600))
                .user("exporter-389ds-rs"),
        )?;

    common_rpm_build(pkg, &exporter_pkg.name)?;

    Ok(())
}

fn copy_binaries(binaries: &[&str]) -> Result<()> {
    let root = get_project_root()?;
    let target = root.join("target").join(MUSL_DIR).join("release");

    for binary in binaries {
        let src = target.join(binary);
        let dest_filename = format!("{}.tar.gz", binary);
        let dest = root.join("target").join("dist").join(&dest_filename);
        let dest_file = std::fs::File::create(&dest)?;
        let enc = flate2::write::GzEncoder::new(&dest_file, flate2::Compression::default());
        let mut builder = tar::Builder::new(enc);
        builder.append_path_with_name(&src, dest_filename)?;
    }

    Ok(())
}

fn build_binaries(binaries: &[&str]) -> Result<()> {
    let sh = Shell::new()?;

    let projects: Vec<String> = binaries.iter().map(|x| format!("-p={}", x)).collect();

    sh.cmd("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "x86_64-unknown-linux-musl",
        ])
        .args(projects)
        .read()?;

    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let binaries = ["nagios-389ds-rs", "exporter-389ds-rs"];

    match args.command {
        CliCommand::Dist => {
            println!("Cleaned cargo");
            build_binaries(&binaries)?;
            println!("Built for musl");
            nagios_389ds_rpm()?;
            println!("Finished packaging nagios");
            exporter_389ds_rpn()?;
            println!("Finished packaging exporter");
            copy_binaries(&binaries)?;
            println!("Copied binaries");
        }
        CliCommand::SetupRepo => {
            let root = get_project_root()?;
            let src = root.join("xtask").join("pre-commit.py");
            let dest = root.join(".git").join("hooks").join("pre-commit");
            std::fs::copy(src, dest)?;
        }
        CliCommand::Fmt => {
            let sh = Shell::new()?;
            cmd!(sh, "cargo fmt").read()?;
            cmd!(sh, "cargo clippy --fix --allow-dirty --allow-staged").read()?;
            cmd!(sh, "taplo format").read()?;
        }
    }

    Ok(())
}
