use crate::commands::AsyncCommandContext;
use crate::utils::FileSystemTree;
use async_zip::base::write::ZipFileWriter;
use async_zip::{Compression, DeflateOption, ZipEntryBuilder};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::fs::File;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::compat::Compat;

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

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct TauriCreateBackupProgress {
    total: usize,
    proceed: usize,
    last_proceed: String,
    #[serde(default)]
    written_bytes: u64,
    #[serde(default)]
    total_bytes: u64,
    #[serde(default)]
    bytes_per_sec: u64,
}

#[derive(Debug)]
pub enum CompressError {
    Io(std::io::Error),
    Zip(async_zip::error::ZipError),
    TaskJoin(tokio::task::JoinError),
    Semaphore(tokio::sync::AcquireError),
}

impl From<std::io::Error> for CompressError {
    fn from(value: std::io::Error) -> Self {
        CompressError::Io(value)
    }
}

impl From<async_zip::error::ZipError> for CompressError {
    fn from(value: async_zip::error::ZipError) -> Self {
        CompressError::Zip(value)
    }
}

impl From<tokio::task::JoinError> for CompressError {
    fn from(value: tokio::task::JoinError) -> Self {
        CompressError::TaskJoin(value)
    }
}

impl From<tokio::sync::AcquireError> for CompressError {
    fn from(value: tokio::sync::AcquireError) -> Self {
        CompressError::Semaphore(value)
    }
}

struct CompressedData {
    bytes: Vec<u8>,
    crc32: u32,
    uncompressed_size: u64,
    _permit: Option<OwnedSemaphorePermit>,
}

struct WriteMessage {
    index: usize,
    relative_path: String,
    data: Option<CompressedData>,
}

impl WriteMessage {
    fn new(index: usize, relative_path: String, data: Option<CompressedData>) -> Self {
        Self {
            index,
            relative_path,
            data,
        }
    }
}

struct WriteState {
    zip: Option<ZipFileWriter<Compat<File>>>,
    compression: Compression,
    deflate_option: DeflateOption,
    next_write_idx: usize,
    pending: BTreeMap<usize, (String, Option<CompressedData>)>,
    rx: tokio::sync::mpsc::UnboundedReceiver<WriteMessage>,
    ctx: AsyncCommandContext<TauriCreateBackupProgress>,
    total_files: usize,
    proceed: Arc<AtomicUsize>,
    total_bytes: u64,
    written_uncompressed: u64,
    written_compressed: u64,
    start_time: std::time::Instant,
    last_emit: std::time::Instant,
}

impl WriteState {
    fn new(
        zip: ZipFileWriter<Compat<File>>,
        compression: Compression,
        deflate_option: DeflateOption,
        rx: tokio::sync::mpsc::UnboundedReceiver<WriteMessage>,
        ctx: AsyncCommandContext<TauriCreateBackupProgress>,
        total_files: usize,
        proceed: Arc<AtomicUsize>,
        total_bytes: u64,
    ) -> Self {
        Self {
            zip: Some(zip),
            compression,
            deflate_option,
            next_write_idx: 0,
            pending: BTreeMap::new(),
            rx,
            ctx,
            total_files,
            proceed,
            total_bytes,
            written_uncompressed: 0,
            written_compressed: 0,
            start_time: std::time::Instant::now(),
            last_emit: std::time::Instant::now(),
        }
    }

    async fn run(mut self) -> Result<(), CompressError> {
        while let Some(msg) = self.rx.recv().await {
            self.submit(msg.index, msg.relative_path, msg.data).await?;
        }
        self.finish().await
    }

    fn emit_writing_progress(&mut self) {
        self.last_emit = std::time::Instant::now();
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let bytes_per_sec = if elapsed > 0.05 {
            (self.written_compressed as f64 / elapsed) as u64
        } else {
            0
        };

        let current_proceed = self.proceed.load(Ordering::Relaxed);
        let written_unc = self.written_uncompressed.min(self.total_bytes);

        let last_proceed = if self.total_bytes > 0 {
            format!(
                "Writing backup archive ({} / {}) [{}/s]",
                format_bytes(written_unc),
                format_bytes(self.total_bytes),
                format_bytes(bytes_per_sec)
            )
        } else {
            format!(
                "Writing backup archive ({}) [{}/s]",
                format_bytes(written_unc),
                format_bytes(bytes_per_sec)
            )
        };

        let _ = self.ctx.emit(TauriCreateBackupProgress {
            total: self.total_files,
            proceed: current_proceed,
            last_proceed,
            written_bytes: written_unc,
            total_bytes: self.total_bytes,
            bytes_per_sec,
        });
    }

    async fn submit(
        &mut self,
        idx: usize,
        relative_path: String,
        data: Option<CompressedData>,
    ) -> Result<(), CompressError> {
        self.pending.insert(idx, (relative_path, data));

        while let Some((name, entry_data)) = self.pending.remove(&self.next_write_idx) {
            if let Some(zip) = self.zip.as_mut() {
                match entry_data {
                    None => {
                        let entry = ZipEntryBuilder::new(name.into(), self.compression)
                            .deflate_option(self.deflate_option);
                        zip.write_entry_whole(entry.build(), b"").await?;
                    }
                    Some(cd) => {
                        let bytes_len = cd.bytes.len() as u64;
                        let uncompressed_size = cd.uncompressed_size;
                        let entry = ZipEntryBuilder::new(name.into(), self.compression)
                            .deflate_option(self.deflate_option)
                            .crc32(cd.crc32)
                            .uncompressed_size(cd.uncompressed_size);
                        zip.write_entry_whole_precompressed(entry.build(), &cd.bytes)
                            .await?;

                        self.written_uncompressed += uncompressed_size;
                        self.written_compressed += bytes_len;
                    }
                }
            }
            self.next_write_idx += 1;
        }

        if self.last_emit.elapsed().as_millis() >= 150 {
            self.emit_writing_progress();
        }

        Ok(())
    }

    async fn finish(&mut self) -> Result<(), CompressError> {
        self.emit_writing_progress();
        if let Some(zip) = self.zip.take() {
            zip.close().await?;
        }
        self.written_uncompressed = self.total_bytes;
        self.emit_writing_progress();
        Ok(())
    }
}

pub(crate) async fn parallel_compress_zip(
    tree: FileSystemTree,
    destination: PathBuf,
    compression: Compression,
    deflate_option: DeflateOption,
    ctx: AsyncCommandContext<TauriCreateBackupProgress>,
) -> Result<(), CompressError> {
    let total = tree.count_all();

    let mut total_bytes: u64 = 0;
    for entry in tree.recursive() {
        if !entry.is_dir() {
            if let Ok(meta) = tokio::fs::metadata(entry.absolute_path()).await {
                total_bytes += meta.len();
            }
        }
    }

    let _ = ctx.emit(TauriCreateBackupProgress {
        total,
        proceed: 0,
        last_proceed: "Collecting files".to_string(),
        written_bytes: 0,
        total_bytes,
        bytes_per_sec: 0,
    });

    let file = File::create_new(&destination).await?;
    let writer = ZipFileWriter::with_tokio(file);

    let proceed = Arc::new(AtomicUsize::new(0));

    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let available_ram = {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();

        // Since the maximum capacity of the semaphore is u32::MAX, it can only handle up to 4GB.
        // To circumvent this, we will use 1 permit for every 10 bytes, allowing for a capacity of up to 40GB.
        let available_ram: u32 = ((sys.free_memory() as f64 / 10.0 * 0.8) as u32) // 80% of free memory
            .max(1);

        log::info!(
            "Using {:.2} GB soft memory limit for compression",
            (available_ram as f64) * 10.0 / 1024.0 / 1024.0 / 1024.0
        );

        available_ram
    };

    let thread_semaphore = Arc::new(Semaphore::new(threads));
    let ram_semaphore = Arc::new(Semaphore::new(available_ram as usize));

    let (sender, rx) = tokio::sync::mpsc::unbounded_channel();
    let write_state = WriteState::new(
        writer,
        compression,
        deflate_option,
        rx,
        ctx.clone(),
        total,
        proceed.clone(),
        total_bytes,
    );

    let merge_task = tokio::spawn(write_state.run());

    let mut handles = vec![];

    for (idx, entry) in tree.recursive().enumerate() {
        if entry.is_dir() {
            let relative_path = entry.relative_path().to_string();
            let _ = sender.send(WriteMessage::new(idx, relative_path.clone(), None));
            let p = proceed.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = ctx.emit(TauriCreateBackupProgress {
                total,
                proceed: p,
                last_proceed: relative_path,
                written_bytes: 0,
                total_bytes,
                bytes_per_sec: 0,
            });
        } else {
            let relative_path = entry.relative_path().to_string();
            let absolute_path = entry.absolute_path().to_path_buf();
            let file_size = tokio::fs::metadata(&absolute_path).await?.len();

            // Permit size is calculated as the number of 10-byte chunks, plus 1 for the remainder.
            // Since memory usage limiting is a soft limit, if the file size exceeds
            // the maximum capacity of the semaphore, fall back to acquiring that maximum capacity.
            let ram_permit_size = ((file_size as f64 / 10.0) as u32)
                .saturating_add(1)
                .min(available_ram);

            let thread_permit = thread_semaphore.clone().acquire_owned().await?;
            let mut ram_permit = ram_semaphore
                .clone()
                .acquire_many_owned(ram_permit_size)
                .await?;

            let sender = sender.clone();
            let ctx = ctx.clone();
            let proceed = proceed.clone();

            let handle: tokio::task::JoinHandle<Result<(), CompressError>> =
                tokio::task::spawn(async move {
                    let (compressed_bytes, crc32, uncompressed_size) = {
                        let raw_data = tokio::fs::read(&absolute_path).await?;
                        let crc32 = async_zip::base::write::crc32(&raw_data);
                        let uncompressed_size = raw_data.len() as u64;

                        let bytes = match compression {
                            Compression::Stored => raw_data,
                            _ => {
                                async_zip::base::write::compress(
                                    &ZipEntryBuilder::new(
                                        relative_path.clone().into(),
                                        compression,
                                    )
                                    .deflate_option(deflate_option)
                                    .build(),
                                    &raw_data,
                                )
                                .await
                            }
                        };

                        (bytes, crc32, uncompressed_size)
                    };

                    drop(thread_permit);

                    // split semaphore and release unused permits
                    let remain_permit =
                        if let Some(new_permits) = ram_permit.split(compressed_bytes.len()) {
                            drop(ram_permit);
                            new_permits
                        } else {
                            // split() returns None if the compressed size exceeds available permits.
                            // This happens when a file is larger than the semaphore's max capacity,
                            // which is allowed as a soft limit at enqueue time. Keep all permits as-is
                            // rather than acquiring new ones, since doing so could deadlock.
                            ram_permit
                        };

                    let compressed_data = CompressedData {
                        bytes: compressed_bytes,
                        crc32,
                        uncompressed_size,
                        _permit: Some(remain_permit),
                    };

                    let _ = sender.send(WriteMessage::new(
                        idx,
                        relative_path.clone(),
                        Some(compressed_data),
                    ));

                    let p = proceed.fetch_add(1, Ordering::Relaxed) + 1;
                    let _ = ctx.emit(TauriCreateBackupProgress {
                        total,
                        proceed: p,
                        last_proceed: relative_path,
                        written_bytes: 0,
                        total_bytes,
                        bytes_per_sec: 0,
                    });

                    Ok(())
                });

            handles.push(handle);
        }
    }

    drop(sender);

    for handle in handles {
        handle.await??;
    }

    merge_task.await??;

    Ok(())
}
