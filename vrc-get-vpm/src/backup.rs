use async_zip::base::read::seek::ZipFileReader;
use futures::AsyncReadExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tokio::io::BufReader;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlcomSystemInfo {
	pub os: String,
	pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlcomBackupSettings {
	pub backup_format: String,
	pub exclude_vpm_packages_from_backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlcomPackageInfo {
	pub name: String,
	pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlcomBackupMetadata {
	pub version: u32,
	pub created_at: u64,
	pub created_at_iso: String,
	pub alcom_version: String,
	pub project_name: String,
	pub project_path: String,
	pub backup_path: String,
	pub unity_version: Option<String>,
	pub vpm_dependencies: BTreeMap<String, String>,
	pub installed_packages: Vec<AlcomPackageInfo>,
	pub system_info: AlcomSystemInfo,
	pub settings: AlcomBackupSettings,
}

/// Reads `alcom.backup.json` from the root of a ZIP backup archive.
pub async fn read_backup_metadata(path: &Path) -> Result<Option<AlcomBackupMetadata>, std::io::Error> {
	if !path.exists() {
		return Ok(None);
	}

	let file = match tokio::fs::File::open(path).await {
		Ok(f) => f,
		Err(_) => return Ok(None),
	};

	let reader = BufReader::new(file).compat();
	let mut zip_reader = match ZipFileReader::new(reader).await {
		Ok(r) => r,
		Err(_) => return Ok(None),
	};

	let total_entries = zip_reader.file().entries().len();
	for idx in 0..total_entries {
		let filename = match zip_reader.file().entries()[idx].filename().as_str() {
			Ok(name) => name.to_string(),
			Err(_) => continue,
		};

		if filename == "alcom.backup.json" || filename.ends_with("/alcom.backup.json") {
			let mut entry_reader = match zip_reader.reader_without_entry(idx).await {
				Ok(r) => r,
				Err(_) => continue,
			};
			let mut string_buf = String::new();
			if entry_reader.read_to_string(&mut string_buf).await.is_ok() {
				if let Ok(meta) = serde_json::from_str::<AlcomBackupMetadata>(&string_buf) {
					return Ok(Some(meta));
				}
			}
		}
	}

	Ok(None)
}
