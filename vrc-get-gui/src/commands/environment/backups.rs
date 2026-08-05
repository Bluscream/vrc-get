use crate::commands::async_command::*;
use crate::commands::prelude::*;
use crate::commands::project::TauriPendingProjectChanges;
use crate::utils::{default_project_path, project_backup_path};
use specta::Type;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State, Window};
use tokio_util::compat::TokioAsyncReadCompatExt;
use vrc_get_vpm::io::DefaultEnvironmentIo;

fn format_bytes(bytes: u64) -> String {
	const K: f64 = 1024.0;
	let b = bytes as f64;
	if b < K {
		format!("{bytes} B")
	} else if b < K * K {
		format!("{:.2} KB", b / K)
	} else if b < K * K * K {
		format!("{:.2} MB", b / (K * K))
	} else {
		format!("{:.2} GB", b / (K * K * K))
	}
}

#[derive(Debug, Clone, serde::Serialize, Type)]
pub struct TauriRestoreBackupProgress {
	pub total: usize,
	pub proceed: usize,
	pub last_proceed: String,
	#[serde(default)]
	pub read_bytes: u64,
	#[serde(default)]
	pub total_bytes: u64,
	#[serde(default)]
	pub bytes_per_sec: u64,
}

#[derive(serde::Serialize, Type)]
pub struct TauriBackupInfo {
	pub file_name: String,
	pub path: String,
	pub size_bytes: u64,
	pub last_modified: u64,
}

#[derive(Clone, serde::Serialize, Type)]
pub struct TauriRestoreResult {
	pub dest_path: String,
	pub should_resolve: bool,
	pub pending_changes: Option<TauriPendingProjectChanges>,
	pub missing_dependencies: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn environment_read_backup_metadata(
	backup_path: String,
) -> Result<Option<vrc_get_vpm::backup::AlcomBackupMetadata>, RustError> {
	let path = PathBuf::from(backup_path);
	let metadata = vrc_get_vpm::backup::read_backup_metadata(&path)
		.await
		.map_err(|e| RustError::unrecoverable_str(format!("Failed to read metadata: {e}")))?;
	Ok(metadata)
}

#[tauri::command]
#[specta::specta]
pub async fn environment_delete_backup(
	backup_path: String,
) -> Result<(), RustError> {
	let path = Path::new(&backup_path);
	if !path.exists() {
		return Err(RustError::unrecoverable_str(format!(
			"Backup file does not exist: {backup_path}"
		)));
	}
	tokio::fs::remove_file(path).await?;
	Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn environment_list_backups(
	settings: State<'_, SettingsState>,
	io: State<'_, DefaultEnvironmentIo>,
) -> Result<Vec<TauriBackupInfo>, RustError> {
	let mut settings = settings.load_mut(io.inner()).await?;
	let backup_dir = project_backup_path(&mut settings).to_string();
	settings.maybe_save().await?;

	let backup_path = Path::new(&backup_dir);
	if !backup_path.exists() {
		return Ok(Vec::new());
	}

	let mut result = Vec::new();
	let mut read_dir = tokio::fs::read_dir(backup_path).await?;
	while let Some(entry) = read_dir.next_entry().await? {
		let path = entry.path();
		if path.is_file() {
			if let Some(ext) = path.extension() {
				let ext_str = ext.to_string_lossy();
				if ext_str.eq_ignore_ascii_case("zip")
					|| ext_str.eq_ignore_ascii_case("tar")
					|| ext_str.eq_ignore_ascii_case("gz")
				{
					let metadata = entry.metadata().await?;
					let file_name = entry.file_name().to_string_lossy().to_string();
					let last_modified = metadata
						.modified()
						.ok()
						.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
						.map(|d| d.as_secs())
						.unwrap_or(0);
					result.push(TauriBackupInfo {
						file_name,
						path: path.to_string_lossy().to_string(),
						size_bytes: metadata.len(),
						last_modified,
					});
				}
			}
		}
	}

	result.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
	Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn environment_restore_backup(
	app_handle: AppHandle,
	window: Window,
	channel: String,
	zip_path: String,
	custom_name: Option<String>,
) -> Result<AsyncCallResult<TauriRestoreBackupProgress, TauriRestoreResult>, RustError> {
	async_command(channel, window, async move {
		With::<TauriRestoreBackupProgress>::continue_async(move |ctx| async move {
			let settings = ctx.state::<SettingsState>();
			let packages = ctx.state::<PackagesState>();
			let changes_state = ctx.state::<ChangesState>();
			let io = ctx.state::<DefaultEnvironmentIo>();
			let http = ctx.state::<reqwest::Client>();

			let mut settings_mut = settings.load_mut(io.inner()).await?;
			let default_proj_path = default_project_path(&mut settings_mut).to_string();

			let zip_file_path = Path::new(&zip_path);
			if !zip_file_path.exists() {
				return Err(RustError::unrecoverable_str(format!(
					"Backup file does not exist: {zip_path}"
				)));
			}

			let zip_file_size = match tokio::fs::metadata(zip_file_path).await {
				Ok(m) => m.len(),
				Err(_) => 0,
			};

			let stem = zip_file_path
				.file_stem()
				.map(|s| s.to_string_lossy().to_string())
				.unwrap_or_else(|| "RestoredProject".to_string());

			let folder_name = custom_name.unwrap_or(stem);
			let dest_dir = Path::new(&default_proj_path).join(&folder_name);

			if dest_dir.exists() {
				return Err(RustError::unrecoverable_str(format!(
					"Target project directory already exists: {}",
					dest_dir.display()
				)));
			}

			tokio::fs::create_dir_all(&dest_dir).await?;

			let file = tokio::fs::File::open(zip_file_path).await?;
			let buf_reader = futures::io::BufReader::new(file.compat());

			let start_time = std::time::Instant::now();
			let mut last_emit = std::time::Instant::now();

			let ctx_clone = ctx.clone();
			let proj_io = vrc_get_vpm::io::DefaultProjectIo::new(dest_dir.clone().into_boxed_path());

			vrc_get_vpm::utils::extract_zip_with_progress(
				buf_reader,
				&proj_io,
				Path::new(""),
				|proceed, total, filename, uncompressed_read| {
					if last_emit.elapsed().as_millis() >= 150 || proceed == total {
						last_emit = std::time::Instant::now();
						let elapsed = start_time.elapsed().as_secs_f64();
						let bytes_per_sec = if elapsed > 0.05 {
							(uncompressed_read as f64 / elapsed) as u64
						} else {
							0
						};

						let last_proceed = if zip_file_size > 0 {
							format!(
								"Extracting backup archive ({}/{}) [{}/s]",
								format_bytes(uncompressed_read),
								format_bytes(zip_file_size),
								format_bytes(bytes_per_sec)
							)
						} else {
							format!("Extracting {filename}")
						};

						let _ = ctx_clone.emit(TauriRestoreBackupProgress {
							total,
							proceed,
							last_proceed,
							read_bytes: uncompressed_read,
							total_bytes: zip_file_size,
							bytes_per_sec,
						});
					}
				},
			)
			.await?;

			let dest_path_str = dest_dir.to_string_lossy().to_string();
			settings_mut.add_user_project(&dest_path_str);
			settings_mut.save().await?;

			let unity_project = load_project(dest_path_str.clone()).await?;
			if unity_project.should_resolve() {
				let settings_guard = settings.load(&io).await?;
				let packages_guard = packages.load(&settings_guard, &io, &http, app_handle).await?;
				let missing_deps = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
				let missing_deps_clone = missing_deps.clone();

				let pending_changes_res = changes_state
					.build_changes_no_list::<_, std::convert::Infallible, _, _>(
						&packages_guard,
						|collection| async move {
							let (changes, missing) = unity_project.resolve_request_partial(collection).await;
							*missing_deps_clone.lock().unwrap() = missing;
							Ok(changes)
						},
						TauriPendingProjectChanges::new,
					)
					.await;

				let missing_vec = std::mem::take(&mut *missing_deps.lock().unwrap());
				let missing_strings = missing_vec
					.into_iter()
					.map(|(dep, range)| format!("{dep}@{range}"))
					.collect();

				let pending_changes = pending_changes_res.ok().filter(|c| !c.is_empty());

				Ok(TauriRestoreResult {
					dest_path: dest_path_str,
					should_resolve: true,
					pending_changes,
					missing_dependencies: missing_strings,
				})
			} else {
				Ok(TauriRestoreResult {
					dest_path: dest_path_str,
					should_resolve: false,
					pending_changes: None,
					missing_dependencies: Vec::new(),
				})
			}
		})
	})
	.await
}
