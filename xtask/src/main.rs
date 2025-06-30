use std::path::PathBuf;
use xtask_toolkit::cargo::{get_project_root, CargoToml};
use xtask_toolkit::checksums::ChecksumsToFile;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use xshell::{cmd, Shell};

const COMMON_GROUP: &str = "o11y-389ds-rs";
const MUSL_DIR: &str = "x86_64-unknown-linux-musl";
const MISC_DIR: &str = "misc";

#[derive(Subcommand, Clone, Debug)]
pub enum CliCommand {
    Dist,

    /// fmt taplo, rust and clippy
    Fmt,

    /// Create pre-commit hooks with gitleaks
    SetupRepo,

    /// Build binaries, create tag and push them as the release
    Release {
        #[arg(short, long, default_value = "false")]
        allow_tagged: bool,

        #[arg(short, long, default_value = "false")]
        skip_tagging: bool,

        #[arg(short, long)]
        binaries: Vec<String>,
    },
}

#[derive(Clone, Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, serde::Deserialize)]
pub struct Package {
    pub version: String,
    pub name: String,
    pub license: String,
    pub description: String,
}

fn common_rpm_build(
    config: &GeneralConfig,
    cargo_toml: &CargoToml,
    pkg: rpm::PackageBuilder,
) -> Result<()> {
    let filename = format!(
        "{}.{}.rpm",
        cargo_toml.versioned_name().unwrap(),
        std::env::consts::ARCH
    );
    std::fs::create_dir_all(&config.dist_files_dir)?;
    pkg.build()?
        .write_file(config.dist_files_dir.join(filename))?;

    Ok(())
}

fn nagios_389ds_rpm(config: &GeneralConfig) -> Result<()> {
    let misc_path = get_project_root()?.join(MISC_DIR);

    let cargo_toml = config.nagios_project();

    let rpm_builder = xtask_toolkit::package_rpm::Package::new(cargo_toml.clone())
        .with_binary_destination("/usr/lib64/nagios/plugins/")
        .with_binary_filename("check_389ds_rs")
        .with_binary_src_archname(MUSL_DIR)
        .builder()?
        .with_file(
            misc_path.join("nagios.sudoers"),
            rpm::FileOptions::new("/etc/sudoers.d/nagios-389ds-rs")
                .mode(rpm::FileMode::regular(0o440))
                .user("root"),
        )?;

    common_rpm_build(config, cargo_toml, rpm_builder)?;

    Ok(())
}

fn exporter_389ds_rpm(config: &GeneralConfig) -> Result<()> {
    let root_dir = get_project_root()?;
    let misc_path = root_dir.join(MISC_DIR);
    let cargo_toml = config.exporter_project();

    let rpm_builder = xtask_toolkit::package_rpm::Package::new(cargo_toml.clone())
        .with_binary_src_archname(MUSL_DIR)
        .with_user("exporter-389ds-rs".to_string())
        .with_group(COMMON_GROUP)
        .with_systemd_unit(misc_path.join("exporter-389ds-rs.service"))
        .expect("Could not find systemd unit file")
        .builder()?
        .with_file(
            misc_path.join("exporter.sudoers"),
            rpm::FileOptions::new("/etc/sudoers.d/exporter-389ds-rs")
                .mode(rpm::FileMode::regular(0o440))
                .user("root"),
        )?
        .with_file(
            misc_path.join("exporter-389ds-rs.minimal.toml"),
            rpm::FileOptions::new("/etc/o11y-389ds-rs/exporter.example.toml")
                .is_config_noreplace()
                .mode(rpm::FileMode::regular(0o600))
                .user("exporter-389ds-rs"),
        )?;

    common_rpm_build(config, cargo_toml, rpm_builder)?;

    Ok(())
}

fn haproxy_389ds_rpm(config: &GeneralConfig) -> Result<()> {
    let root_dir = get_project_root()?;
    let misc_path = root_dir.join(MISC_DIR);
    let cargo_toml = config.haproxy_project();

    let rpm_builder = xtask_toolkit::package_rpm::Package::new(cargo_toml.clone())
        .with_binary_src_archname(MUSL_DIR)
        .with_user("haproxy-389ds-rs")
        .with_group(COMMON_GROUP)
        .with_systemd_unit(misc_path.join("haproxy-389ds-rs.service"))
        .expect("Could not find systemd unit file")
        .builder()?
        .with_file(
            misc_path.join("haproxy.sudoers"),
            rpm::FileOptions::new("/etc/sudoers.d/haproxy-389ds-rs")
                .mode(rpm::FileMode::regular(0o440))
                .user("root"),
        )?
        .with_file(
            misc_path.join("haproxy-389ds-rs.minimal.toml"),
            rpm::FileOptions::new("/etc/o11y-389ds-rs/haproxy.example.toml")
                .is_config_noreplace()
                .mode(rpm::FileMode::regular(0o600))
                .user("haproxy-389ds-rs"),
        )?;

    common_rpm_build(config, cargo_toml, rpm_builder)?;

    Ok(())
}

fn config_389ds_rpm(config: &GeneralConfig) -> Result<()> {
    let root_dir = get_project_root()?;
    let misc_path = root_dir.join(MISC_DIR);
    let cargo_toml = config.config_project();

    let rpm_builder = xtask_toolkit::package_rpm::Package::new(cargo_toml.clone())
        .dont_include_binary()
        .keep_file_after_removal("/etc/o11y-389ds-rs/default.toml")
        .with_group(COMMON_GROUP)
        .builder()?
        .with_file(
            misc_path.join("default.toml"),
            rpm::FileOptions::new("/etc/o11y-389ds-rs/default.toml")
                .is_config_noreplace()
                .mode(rpm::FileMode::regular(0o640))
                .user("root")
                .group("o11y-389ds-rs"),
        )?;

    common_rpm_build(config, cargo_toml, rpm_builder)?;

    Ok(())
}

fn copy_binaries(config: &GeneralConfig) -> Result<()> {
    for binary_name in &config.binaries {
        let dist = config.targz_path(
            &config
                .projects
                .iter()
                .find(|x| x.name().is_some_and(|x| &x == binary_name))
                .ok_or(anyhow!("Could not find binary {binary_name}"))?
                .versioned_name()
                .unwrap_or(binary_name.to_string()),
        );
        xtask_toolkit::targz::DirCompress::new(&config.release_target_dir)
            .expect("Could not create compressor")
            .filter_filename(binary_name)
            .compress(&dist)?;
    }

    Ok(())
}

fn generate_checksums_new(config: &GeneralConfig) -> Result<()> {
    let mut files_checksums = xtask_toolkit::checksums::PathChecksum::calculate_entries_sha256(
        config.dist_files_dir.as_path(),
    )?;

    for project in &config.projects {
        if let Some(subproject_name) = project.versioned_name() {
            let code_checksum = xtask_toolkit::checksums::PathChecksum::calculate_sha256_filtered(
                project.path().parent().unwrap(),
                |p| {
                    p.file_name().is_some_and(|x| {
                        !x.to_string_lossy().starts_with(".") && x.to_string_lossy() != "target"
                    })
                },
            )?;
            files_checksums.insert(format!("CODE[{}]", subproject_name), code_checksum);
        }
    }

    let grafana_checksums = xtask_toolkit::checksums::PathChecksum::calculate_entries_sha256(
        config
            .grafana_project()
            .path()
            .parent()
            .expect("Could not get grafana dir"),
    )?;

    files_checksums.extend(grafana_checksums.into_iter().filter_map(|(x, y)| {
        (!x.starts_with(".")).then_some((format!("grafana-389ds-rs/{x}"), y))
    }));

    for project in &config.projects {
        if let Some(package_name) = project.versioned_name().and_then(|_| project.name()) {
            let mut checksums = files_checksums
                .iter()
                .filter_map(|(k, v)| {
                    ((k.starts_with(&package_name)
                        || k.starts_with("CODE[internal")
                        || (k.contains(&package_name) && k.starts_with("CODE")))
                        && !k.ends_with("sha256"))
                    .then_some((k.clone(), v.clone()))
                })
                .collect::<Vec<_>>();

            checksums.sort_by_key(|(k, _)| k.to_string());

            checksums.into_iter().save_checksum(
                &config
                    .dist_files_dir
                    .join(format!("{package_name}.{}.sha256", std::env::consts::ARCH)),
            )?;
        }
    }

    Ok(())
}

fn release_binary(name: &str, version: &str, files: Vec<PathBuf>) -> Result<()> {
    let files: Vec<String> = files
        .iter()
        .map(|x| x.to_string_lossy().to_string())
        .collect();

    println!(
        "{}",
        xtask_toolkit::gh_cli::Release::new(name, version)?.release(&files)?
    );

    Ok(())
}

fn compress_grafana_dashboards(config: &GeneralConfig) -> Result<()> {
    let grafana_dir = config
        .grafana_project()
        .path()
        .parent()
        .unwrap()
        .to_path_buf();

    xtask_toolkit::targz::DirCompress::new(&grafana_dir)
        .ok_or(anyhow!(
            "Could not create compressor for grafana dashboards - dir {:?} does not exist",
            grafana_dir
        ))?
        .filter_extension(".json")
        .compress(
            &config.targz_path(
                &config.grafana_project().versioned_name().unwrap_or(
                    config
                        .grafana_project()
                        .name()
                        .expect("Could not get project name"),
                ),
            ),
        )?;

    Ok(())
}

/// Check if the tag already exists
const BINARIES: &[&str] = &["nagios-389ds-rs", "exporter-389ds-rs", "haproxy-389ds-rs"];
const OTHER_PROJECTS: &[&str] = &["grafana-389ds-rs", "config-389ds-rs"];

pub struct GeneralConfig {
    pub projects: Vec<CargoToml>,

    pub project_root: PathBuf,

    /// Default dir for cargo builds
    pub release_target_dir: PathBuf,

    /// Directory with all files for the distribution
    pub dist_files_dir: PathBuf,

    pub binaries: Vec<String>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl GeneralConfig {
    pub fn new() -> Self {
        let projects = CargoToml::autodiscovery_with(&["project.toml"]);
        let project_root = get_project_root().unwrap();

        Self {
            projects,
            release_target_dir: project_root.join("target").join(MUSL_DIR).join("release"),
            dist_files_dir: project_root.join("target").join("dist"),
            project_root,
            binaries: BINARIES.iter().map(|x| x.to_string()).collect(),
        }
    }

    pub fn config_project(&self) -> &CargoToml {
        self.projects
            .iter()
            .find(|x| x.name().is_some_and(|x| x == "config-389ds-rs"))
            .expect("o11y-389ds-rs not found")
    }

    pub fn exporter_project(&self) -> &CargoToml {
        self.projects
            .iter()
            .find(|x| x.name().is_some_and(|x| x == "exporter-389ds-rs"))
            .expect("exporter-389ds-rs not found")
    }

    pub fn grafana_project(&self) -> &CargoToml {
        self.projects
            .iter()
            .find(|x| x.name().is_some_and(|x| x == "grafana-389ds-rs"))
            .expect("grafana-389ds-rs not found")
    }

    pub fn nagios_project(&self) -> &CargoToml {
        self.projects
            .iter()
            .find(|x| x.name().is_some_and(|x| x == "nagios-389ds-rs"))
            .expect("nagios-389ds-rs not found")
    }

    pub fn haproxy_project(&self) -> &CargoToml {
        self.projects
            .iter()
            .find(|x| x.name().is_some_and(|x| x == "haproxy-389ds-rs"))
            .expect("haproxy-389ds-rs not found")
    }

    pub fn targz_path(&self, name: &str) -> PathBuf {
        self.dist_files_dir
            .join(format!("{}.{}.tar.gz", name, std::env::consts::ARCH))
    }
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let general_config = GeneralConfig::new();

    match args.command {
        CliCommand::Dist => {
            xtask_toolkit::cargo::BinaryBuild::new()
                .with_projects(&general_config.binaries)
                .with_target(MUSL_DIR)
                .build()
                .inspect_err(|_| println!("Failed to build for musl"))
                .inspect(|_| println!("Built for musl"))?;

            config_389ds_rpm(&general_config)
                .inspect_err(|_| println!("Failed to package config"))
                .inspect(|_| println!("Finished packaging config"))?;

            nagios_389ds_rpm(&general_config)
                .inspect_err(|_| println!("Failed to package nagios"))
                .inspect(|_| println!("Finished packaging nagios"))?;

            exporter_389ds_rpm(&general_config)
                .inspect_err(|_| println!("Failed to package exporter"))
                .inspect(|_| println!("Finished packaging exporter"))?;

            haproxy_389ds_rpm(&general_config)
                .inspect_err(|_| println!("Failed to package haproxy"))
                .inspect(|_| println!("Finished packaging haproxy"))?;

            copy_binaries(&general_config)
                .inspect_err(|_| println!("Failed to copy binaries"))
                .inspect(|_| println!("Copied binaries"))?;

            compress_grafana_dashboards(&general_config)
                .inspect_err(|_| println!("Failed to compress grafana dashboards"))
                .inspect(|_| println!("Compressed grafana dashboards"))?;

            generate_checksums_new(&general_config)?;
            println!("Generated checksums");
        }
        CliCommand::SetupRepo => {
            xtask_toolkit::precommit::install_precommit(Default::default())?;
        }
        CliCommand::Fmt => {
            xtask_toolkit::cargo::force_fmt()?;
            let sh = Shell::new()?;
            cmd!(sh, "taplo format").read()?;
        }
        CliCommand::Release {
            allow_tagged,
            skip_tagging,
            binaries,
        } => {
            if xtask_toolkit::git::unstaged_changes()? {
                return Err(anyhow!("There are unstaged changes. Stash or commit them"));
            }

            let already_released = xtask_toolkit::gh_cli::Release::get_from_gh()?;
            let mut errors = Vec::new();

            for binary in BINARIES.iter().chain(OTHER_PROJECTS) {
                let project = general_config
                    .projects
                    .iter()
                    .find(|x| x.name().as_deref() == Some(binary))
                    .unwrap();

                let versioned_binary = project.versioned_name().unwrap();
                let project_name = project.name().unwrap();
                let project_version = project.version().unwrap();

                if !(binaries.is_empty() || binaries.contains(&binary.to_string())) {
                    continue;
                }

                println!("Releasing {versioned_binary}");

                if already_released.iter().any(|released| {
                    released.name == project_name && released.version.to_string() == project_version
                }) {
                    println!("{} has been already released", versioned_binary);
                    continue;
                }

                let files: Vec<PathBuf> = general_config
                    .dist_files_dir
                    .read_dir()?
                    .filter_map(|entry| {
                        entry.ok().and_then(|path| {
                            path.file_name()
                                .to_string_lossy()
                                .starts_with(&project_name)
                                .then_some(path.path())
                        })
                    })
                    .collect();

                if files.is_empty() {
                    println!("No files to attach to the release ({})", &binary);
                    continue;
                }

                let already_tagged = xtask_toolkit::git::has_tag(&versioned_binary)?;

                if already_tagged && !allow_tagged {
                    println!("Tag already exists. Use --allow-tagged to force the release");
                    continue;
                } else if !skip_tagging {
                    xtask_toolkit::git::create_and_push_tag(&versioned_binary)?;
                }

                errors.push(release_binary(&project_name, &project_version, files));
            }
            for err in errors {
                err?;
            }
        }
    }

    Ok(())
}
