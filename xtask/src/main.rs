mod checksum;

use sha2::{Digest, Sha256};
use std::{env, ffi::OsString, fs::read_dir, path::PathBuf};

use chrono::DateTime;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use xshell::{cmd, Shell};

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
    let binary = Binary::from_version(rpm_name)?;
    let result_path = binary.rpm_path()?;

    std::fs::create_dir_all(result_path.parent().unwrap())?;
    pkg.build()?.write_file(result_path)?;

    Ok(())
}

fn nagios_389ds_rpm() -> Result<()> {
    let nagios_pkg = read_package("nagios-389ds-rs")?;
    let binary_location = Binary::from(&nagios_pkg).binary_path()?;

    let misc_path = get_project_root()?.join(MISC_DIR);

    let pkg = common_rpm(&nagios_pkg)?
        .with_file(
            binary_location,
            rpm::FileOptions::new("/usr/lib64/nagios/plugins/check_389ds_rs")
                .mode(rpm::FileMode::regular(0o755)),
        )?
        .with_file(
            misc_path.join("nagios.sudoers"),
            rpm::FileOptions::new("/etc/sudoers.d/nagios-389ds-rs")
                .mode(rpm::FileMode::regular(0o440))
                .user("root"),
        )?;

    common_rpm_build(pkg, &nagios_pkg.name)?;

    Ok(())
}

fn exporter_389ds_rpn() -> Result<()> {
    let exporter_pkg = read_package("exporter-389ds-rs")?;
    let binary_location = Binary::from(&exporter_pkg).binary_path()?;
    let root_dir = get_project_root()?;

    const PRE_INSTAL_SCRIPT: &str = r#"
        if [ -z "$(getent passwd | grep exporter-389ds-rs)" ]; then 
            useradd -r exporter-389ds-rs; 
        fi
    "#;

    const POST_INSTALL_SCRIPT: &str = r#"
        systemctl daemon-reload; 
    "#;

    const PRE_UNINSTALL_SCRIPT: &str = r#"
        IS_UPGRADED="$1"
        case "$IS_UPGRADED" in
           0) # This is a yum remove.
              if [ -n "$(getent passwd | grep exporter-389ds-rs)" ]; then 
                  systemctl disable exporter-389ds-rs.service;
                  systemctl stop exporter-389ds-rs.service;
                  userdel exporter-389ds-rs;
              fi
           ;;
           1) # This is a yum upgrade.
              systemctl is-active --quiet exporter-389ds-rs && systemctl restart exporter-389ds-rs;
              exit 0;
           ;;
         esac
    "#;

    const POST_UNINSTALL_SCRIPT: &str = r#"
        systemctl daemon-reload;
    "#;

    let misc_path = root_dir.join(MISC_DIR);

    let pkg = common_rpm(&exporter_pkg)?
        .pre_install_script(PRE_INSTAL_SCRIPT)
        .post_install_script(POST_INSTALL_SCRIPT)
        .pre_uninstall_script(PRE_UNINSTALL_SCRIPT)
        .post_uninstall_script(POST_UNINSTALL_SCRIPT)
        .with_file(
            binary_location,
            rpm::FileOptions::new("/usr/bin/exporter-389ds-rs").mode(rpm::FileMode::regular(0o755)),
        )?
        .with_file(
            misc_path.join("exporter-389ds-rs.service"),
            rpm::FileOptions::new("/etc/systemd/system/exporter-389ds-rs.service")
                .mode(rpm::FileMode::regular(0o644)),
        )?
        .with_file(
            misc_path.join("exporter.sudoers"),
            rpm::FileOptions::new("/etc/sudoers.d/exporter-389ds-rs")
                .mode(rpm::FileMode::regular(0o440))
                .user("root"),
        )?
        .with_file(
            misc_path.join("exporter-389ds-rs.minimal.toml"),
            rpm::FileOptions::new("/etc/o11y-389ds-rs/default.toml")
                .is_config_noreplace()
                .mode(rpm::FileMode::regular(0o600))
                .user("exporter-389ds-rs"),
        )?;

    common_rpm_build(pkg, &exporter_pkg.name)?;

    Ok(())
}

fn copy_binaries(binaries: &[&str]) -> Result<()> {
    for binary_name in binaries {
        Binary::from_version(binary_name)?.compress_binary_to_dist()?;
    }
    Ok(())
}

fn generate_checksums(binaries: &[&str]) -> Result<()> {
    for binary_name in binaries {
        Binary::from_version(binary_name)?
            .save_checksum()
            .context(format!("Failed for {binary_name}"))?;
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

pub struct ReleasedVersions(Vec<Binary>);
impl ReleasedVersions {
    pub fn get_from_gh() -> Result<ReleasedVersions> {
        #[derive(serde::Deserialize)]
        pub struct GhResponse {
            #[serde(rename = "tagName")]
            pub tag_name: String,
        }

        let sh = Shell::new()?;
        let previous_releases: Vec<GhResponse> =
            serde_json::from_str(&cmd!(sh, "gh release list --json tagName").read()?)?;

        Ok(Self(
            previous_releases
                .into_iter()
                .filter_map(|x| Binary::try_from(x.tag_name).ok())
                .collect(),
        ))
    }

    pub fn contains(&self, binary: &Binary) -> bool {
        self.0.iter().any(|x| x == binary)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Binary {
    pub name: String,
    pub version: String,
}

impl std::fmt::Display for Binary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.versioned_binary())
    }
}

#[derive(Debug)]
pub struct LabeledDistFile {
    pub path: std::path::PathBuf,
    pub label: Option<String>,
}

impl std::fmt::Display for LabeledDistFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dist_file = if let Some(label) = &self.label {
            format!("{}#{}", self.path.display(), label)
        } else {
            self.path.display().to_string()
        };
        f.write_str(&dist_file)
    }
}

impl Binary {
    pub fn compress_binary_to_dist(&self) -> Result<()> {
        let src = self.binary_path()?;
        let dest = self.targz_path()?;

        let dest_file = std::fs::File::create(&dest)?;
        let enc = flate2::write::GzEncoder::new(&dest_file, flate2::Compression::default());
        let mut builder = tar::Builder::new(enc);
        builder.append_path_with_name(&src, &self.name)?;

        Ok(())
    }

    pub fn compress_project_files_to_dist(&self, files: &[String]) -> Result<()> {
        let dest = self.targz_path()?;
        let dest_file = std::fs::File::create(&dest)?;
        let enc = flate2::write::GzEncoder::new(&dest_file, flate2::Compression::default());
        let mut builder = tar::Builder::new(enc);

        for src in files {
            builder.append_path_with_name(src, &self.name)?;
        }

        Ok(())
    }

    pub fn dist_path(&self) -> Result<PathBuf> {
        let root = get_project_root()?;
        Ok(root.join("target").join("dist"))
    }

    pub fn targz_path(&self) -> Result<PathBuf> {
        let targz_filename = format!("{}.{}.tar.gz", self, std::env::consts::ARCH);
        Ok(self.dist_path()?.join(&targz_filename))
    }

    pub fn binary_path(&self) -> Result<PathBuf> {
        let root = get_project_root()?;
        Ok(root
            .join("target")
            .join(MUSL_DIR)
            .join("release")
            .join(&self.name))
    }

    pub fn rpm_path(&self) -> Result<PathBuf> {
        Ok(self.dist_path()?.join(format!(
            "{}.{}.rpm",
            self.versioned_binary(),
            std::env::consts::ARCH,
        )))
    }

    pub fn sha256_path(&self) -> Result<PathBuf> {
        Ok(self.dist_path()?.join(format!(
            "{}.{}.sha256",
            self.versioned_binary(),
            std::env::consts::ARCH,
        )))
    }

    pub fn code_path(&self) -> Result<PathBuf> {
        let root = get_project_root()?;
        Ok(root.join(&self.name))
    }

    pub fn checksum(&self) -> Result<Vec<(String, PathBuf)>> {
        let mut result = vec![];

        if self.binary_path()?.is_file() {
            let binary_content =
                std::fs::read(&self.binary_path()?).context("Failed reading binary")?;
            let mut hasher = Sha256::new();
            hasher.update(&binary_content);
            let binary_checksum = format!("{:x}", hasher.finalize());

            result.push((binary_checksum, self.binary_path()?));
        }

        let targz_content = std::fs::read(&self.targz_path()?).context("Failed reading targz")?;
        let mut hasher = Sha256::new();
        hasher.update(&targz_content);
        let targz_checksum = format!("{:x}", hasher.finalize());
        result.push((targz_checksum, self.targz_path()?));

        if self.rpm_path()?.is_file() {
            let package_content = std::fs::read(&self.rpm_path()?).context("Failed reading rpm")?;
            let mut hasher = Sha256::new();
            hasher.update(&package_content);
            let package_checksum = format!("{:x}", hasher.finalize());
            result.push((package_checksum, self.rpm_path()?));
        }

        let code_checksum = checksum::calculate_dir_checksum(&self.code_path()?)?;
        result.push((
            code_checksum,
            std::path::PathBuf::from(format!("CODE[{}]", self.name)),
        ));

        let root_dir = get_project_root()?;
        let code_checksum = checksum::calculate_dir_checksum(&root_dir.join("internal"))?;
        result.push((code_checksum, std::path::PathBuf::from("CODE[internal]")));

        Ok(result)
    }

    pub fn save_checksum(&self) -> Result<()> {
        let filepath = self.sha256_path()?;

        let file_contents = self.checksum()?.into_iter().fold(String::new(), |acc, x| {
            format!(
                "{} {} {}\n",
                acc,
                x.0,
                x.1.file_name().unwrap().to_str().unwrap()
            )
        });

        std::fs::write(&filepath, file_contents)?;

        Ok(())
    }

    pub fn get_release_files(&self) -> Result<Vec<LabeledDistFile>> {
        let dist = self.dist_path()?;

        let files = std::fs::read_dir(&dist)?;
        let files: Vec<std::path::PathBuf> = files
            .filter_map(|entry| {
                entry.ok().and_then(|path| {
                    path.file_type()
                        .is_ok_and(|p| p.is_file())
                        .then_some(path.path())
                })
            })
            .filter(|x| {
                x.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains(&self.to_string())
            })
            .collect();

        Ok(files
            .into_iter()
            .map(|path| {
                let filename = path.file_name().unwrap().to_string_lossy().to_string();

                if filename.ends_with(".rpm") && filename.contains("x86_64") {
                    LabeledDistFile {
                        path,
                        label: Some("RPM package (x86-64/amd64)".to_string()),
                    }
                } else if filename.ends_with(".tar.gz") && filename.contains("x86_64") {
                    LabeledDistFile {
                        path,
                        label: Some("Binary (x86-64/amd64)".to_string()),
                    }
                } else {
                    LabeledDistFile { path, label: None }
                }
            })
            .collect())
    }

    fn from_version_file(name: &str, file: &str) -> Result<Self> {
        let cargo_toml_path = get_project_root()?.join(name).join(file);

        let cargo_toml: serde_json::Value =
            toml::from_str(&std::fs::read_to_string(&cargo_toml_path)?)?;

        let version = cargo_toml
            .get("package")
            .ok_or(anyhow!("Could not find package key in the Cargo.toml"))?
            .get("version")
            .ok_or(anyhow!("Could not find version key in the Cargo.toml"))?
            .as_str()
            .ok_or(anyhow!("Could not parse version"))?
            .to_string();

        Ok(Self {
            name: name.to_string(),
            version,
        })
    }

    pub fn from_version(name: &str) -> Result<Self> {
        let root = get_project_root()?.join(name);
        if root.join("Cargo.toml").exists() {
            Self::from_version_file(name, "Cargo.toml")
        } else if root.join("project.toml").exists() {
            Self::from_version_file(name, "project.toml")
        } else {
            Err(anyhow!("Could not find Cargo.toml or project.toml"))
        }
    }

    pub fn create_and_push_tag(&self) -> Result<()> {
        let sh = Shell::new()?;
        let versioned = self.versioned_binary();

        cmd!(sh, "git tag {versioned}").run()?;
        cmd!(sh, "git push origin {versioned}").run()?;
        Ok(())
    }

    pub fn versioned_binary(&self) -> String {
        format!("{}-{}", self.name, self.version)
    }

    fn already_tagged(&self) -> Result<bool> {
        let sh = Shell::new()?;

        let tags = cmd!(sh, "git tag")
            .read()?
            .split('\n')
            .map(|x| x.trim().to_string())
            .collect::<Vec<_>>();

        Ok(tags.contains(&self.versioned_binary()))
    }
}

impl TryFrom<String> for Binary {
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let (name, version) = value.rsplit_once('-').ok_or(())?;

        let version_regex = regex::Regex::new("^[0-9]+\\.[0-9]+\\.[0-9]+$").unwrap();
        if version_regex.is_match(version) {
            Ok(Binary {
                name: name.to_string(),
                version: version.to_string(),
            })
        } else {
            Err(())
        }
    }

    type Error = ();
}

impl From<&Package> for Binary {
    fn from(value: &Package) -> Self {
        Binary {
            name: value.name.clone(),
            version: value.version.clone(),
        }
    }
}

fn release_binary(binary: &Binary, files: Vec<LabeledDistFile>) -> Result<()> {
    let sh = Shell::new()?;
    let mut files = files.into_iter().map(|f| f.to_string());

    println!(
        "{}",
        sh.cmd("gh")
            .args(["release", "create", "--generate-notes"])
            .arg(binary.to_string())
            .args(&mut files)
            .read()?
    );

    Ok(())
}

fn compress_grafana_dashboards() -> Result<()> {
    let grafana_root = get_project_root()?.join("grafana-389ds-rs");

    let files = std::fs::read_dir(grafana_root)?
        .filter_map(|x| {
            x.ok()
                .and_then(|x| {
                    if x.file_name().to_string_lossy().ends_with(".json") {
                        Some(x)
                    } else {
                        None
                    }
                })
                .and_then(|x| x.path().is_file().then_some(x))
        })
        .map(|x| x.path().to_string_lossy().to_string())
        .collect::<Vec<String>>();

    let binary = Binary::from_version("grafana-389ds-rs")?;
    binary.compress_project_files_to_dist(&files)?;

    Ok(())
}

fn unstaged_changes() -> Result<bool> {
    let sh = Shell::new()?;

    Ok(!cmd!(sh, "git status --porcelain").read()?.is_empty())
}

/// Check if the tag already exists
const BINARIES: &[&str] = &["nagios-389ds-rs", "exporter-389ds-rs"];
const OTHER_PROJECTS: &[&str] = &["grafana-389ds-rs"];

fn main() -> Result<()> {
    let args = Cli::parse();

    match args.command {
        CliCommand::Dist => {
            println!("Cleaned cargo");
            build_binaries(BINARIES)?;
            println!("Built for musl");
            nagios_389ds_rpm()?;
            println!("Finished packaging nagios");
            exporter_389ds_rpn()?;
            println!("Finished packaging exporter");
            copy_binaries(BINARIES)?;
            println!("Copied binaries");
            compress_grafana_dashboards()?;
            println!("Compressed grafana dashboards into a single .tar.gz");
            generate_checksums(
                &BINARIES
                    .iter()
                    .chain(OTHER_PROJECTS)
                    .copied()
                    .collect::<Vec<_>>(),
            )?;
            println!("Generated checksums");
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
        CliCommand::Release {
            allow_tagged,
            skip_tagging,
            binaries,
        } => {
            if unstaged_changes()? {
                return Err(anyhow!("There are unstaged changes. Stash or commit them"));
            }

            let already_released = ReleasedVersions::get_from_gh()?;
            let mut errors = Vec::new();

            for binary in BINARIES.iter().chain(OTHER_PROJECTS) {
                let versioned_binary = Binary::from_version(binary)?;

                if !(binaries.is_empty() || binaries.contains(&binary.to_string())) {
                    continue;
                }

                println!("Releasing {versioned_binary}");

                if already_released.contains(&versioned_binary) {
                    println!("{} has been already released", versioned_binary);
                    continue;
                }

                let files = versioned_binary.get_release_files()?;
                if files.is_empty() {
                    println!("No files to attach to the release ({})", &binary);
                    continue;
                }

                if versioned_binary.already_tagged()? && !allow_tagged {
                    println!("Tag already exists. Use --allow-tagged to force the release");
                    continue;
                } else if !skip_tagging {
                    versioned_binary.create_and_push_tag()?;
                }

                errors.push(release_binary(&versioned_binary, files));
            }
            for err in errors {
                err?;
            }
        }
    }

    Ok(())
}
