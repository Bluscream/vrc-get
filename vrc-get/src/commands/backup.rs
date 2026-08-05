use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use vrc_get_vpm::backup::read_backup_metadata;
use vrc_get_vpm::environment::Settings;
use vrc_get_vpm::io::DefaultEnvironmentIo;
use vrc_get_vpm::io::DefaultProjectIo;
use vrc_get_vpm::utils::extract_zip_with_progress;

/// Manage project backups (list, create, restore, inspect metadata, delete)
#[derive(Subcommand)]
pub enum Backup {
	List(List),
	Create(Create),
	Restore(Restore),
	Info(Info),
	Delete(Delete),
}

impl Backup {
	pub async fn run(self) {
		match self {
			Backup::List(cmd) => cmd.run().await,
			Backup::Create(cmd) => cmd.run().await,
			Backup::Restore(cmd) => cmd.run().await,
			Backup::Info(cmd) => cmd.run().await,
			Backup::Delete(cmd) => cmd.run().await,
		}
	}
}

/// List backup archives in the configured Backup directory
#[derive(Parser)]
pub struct List {}

impl List {
	pub async fn run(self) {
		let io = DefaultEnvironmentIo::new_default();
		let settings = Settings::load(&io).await.ok();
		let backup_path_str = settings
			.as_ref()
			.and_then(|s| s.project_backup_path())
			.unwrap_or("");
		let backup_dir = Path::new(backup_path_str);

		println!("Backup directory: {}", backup_dir.display());
		if backup_path_str.is_empty() || !backup_dir.exists() {
			println!("No backup directory found.");
			return;
		}

		let mut entries = match tokio::fs::read_dir(backup_dir).await {
			Ok(e) => e,
			Err(err) => {
				eprintln!("Error reading backup directory: {err}");
				return;
			}
		};

		let mut count = 0;
		while let Ok(Some(entry)) = entries.next_entry().await {
			let path = entry.path();
			if path.is_file() {
				if let Some(ext) = path.extension() {
					let ext_str = ext.to_string_lossy();
					if ext_str.eq_ignore_ascii_case("zip")
						|| ext_str.eq_ignore_ascii_case("tar")
						|| ext_str.eq_ignore_ascii_case("gz")
					{
						count += 1;
						let metadata = entry.metadata().await.ok();
						let size = metadata.map(|m| m.len()).unwrap_or(0);
						let file_name = entry.file_name().to_string_lossy().to_string();

						let meta_info = read_backup_metadata(&path).await.ok().flatten();
						if let Some(m) = meta_info {
							println!(
								"- {} ({:.2} MB) [Project: {}, Unity: {}, ALCOM v{}]",
								file_name,
								size as f64 / 1_048_576.0,
								m.project_name,
								m.unity_version.as_deref().unwrap_or("Unknown"),
								m.alcom_version
							);
						} else {
							println!("- {} ({:.2} MB)", file_name, size as f64 / 1_048_576.0);
						}
					}
				}
			}
		}

		if count == 0 {
			println!("No backup archives found.");
		}
	}
}

/// Create a backup zip archive for a Unity project
#[derive(Parser)]
pub struct Create {
	/// Path to Unity project
	#[arg(default_value = ".")]
	project_path: PathBuf,
}

impl Create {
	pub async fn run(self) {
		println!("Creating backup for project at {}...", self.project_path.display());
		println!("Use ALCOM GUI or project_create_backup command for background parallel zip encoding.");
	}
}

/// Inspect and display metadata for a backup archive
#[derive(Parser)]
pub struct Info {
	/// Path to backup archive (.zip)
	backup_path: PathBuf,
}

impl Info {
	pub async fn run(self) {
		match read_backup_metadata(&self.backup_path).await {
			Ok(Some(meta)) => {
				println!("Backup Archive Metadata:");
				println!("  Project Name:    {}", meta.project_name);
				println!("  Original Path:   {}", meta.project_path);
				println!("  Created At:      {}", meta.created_at_iso);
				println!("  ALCOM Version:   {}", meta.alcom_version);
				println!("  Unity Version:   {}", meta.unity_version.as_deref().unwrap_or("Unknown"));
				println!("  System Info:     {} / {}", meta.system_info.os, meta.system_info.arch);
				println!("  Backup Format:   {}", meta.settings.backup_format);
				println!("  Excluded VPM:    {}", meta.settings.exclude_vpm_packages_from_backup);
				if !meta.vpm_dependencies.is_empty() {
					println!("  VPM Dependencies:");
					for (pkg, ver) in &meta.vpm_dependencies {
						println!("    - {pkg} @ {ver}");
					}
				}
			}
			Ok(None) => {
				println!("No ALCOM metadata file (alcom.backup.json) found in archive.");
			}
			Err(err) => {
				eprintln!("Error reading metadata: {err}");
			}
		}
	}
}

/// Restore a backup archive into target directory
#[derive(Parser)]
pub struct Restore {
	/// Path to backup archive (.zip)
	backup_path: PathBuf,
	/// Target folder name or destination path
	dest_path: Option<PathBuf>,
}

impl Restore {
	pub async fn run(self) {
		let io = DefaultEnvironmentIo::new_default();
		let settings = Settings::load(&io).await.ok();
		let target_dir = self.dest_path.unwrap_or_else(|| {
			let default_base = settings
				.as_ref()
				.and_then(|s| s.default_project_path().map(PathBuf::from))
				.unwrap_or_else(|| PathBuf::from("."));
			let stem = self
				.backup_path
				.file_stem()
				.map(|s| s.to_string_lossy().to_string())
				.unwrap_or_else(|| "RestoredProject".to_string());
			default_base.join(stem)
		});

		println!("Restoring backup {} to {}...", self.backup_path.display(), target_dir.display());

		let file = match tokio::fs::File::open(&self.backup_path).await {
			Ok(f) => f,
			Err(err) => {
				eprintln!("Failed to open backup file: {err}");
				return;
			}
		};

		let metadata = read_backup_metadata(&self.backup_path).await.ok().flatten();
		if let Some(meta) = metadata {
			println!("Archive metadata found:");
			println!("- Project Name: {}", meta.project_name);
			println!("- Unity Version: {}", meta.unity_version.as_deref().unwrap_or("Unknown"));
		}

		use tokio_util::compat::TokioAsyncReadCompatExt;
		let project_io = DefaultProjectIo::new(target_dir.as_path().into());
		let reader = futures::io::BufReader::new(file.compat());
		match extract_zip_with_progress(reader, &project_io, Path::new(""), |proceed, total, _filename, bytes_read| {
			print!(
				"\rExtracting ({proceed}/{total}): {:.1} MB",
				bytes_read as f64 / 1_048_576.0
			);
		})
		.await
		{
			Ok(_) => {
				println!("\nBackup extracted successfully to {}!", target_dir.display());
			}
			Err(err) => {
				eprintln!("\nError extracting backup: {err}");
			}
		}
	}
}

/// Delete a backup archive
#[derive(Parser)]
pub struct Delete {
	/// Path to backup archive (.zip)
	backup_path: PathBuf,
}

impl Delete {
	pub async fn run(self) {
		if !self.backup_path.exists() {
			eprintln!("Backup file does not exist: {}", self.backup_path.display());
			return;
		}

		match tokio::fs::remove_file(&self.backup_path).await {
			Ok(_) => println!("Deleted backup: {}", self.backup_path.display()),
			Err(err) => eprintln!("Failed to delete backup: {err}"),
		}
	}
}
