use crate::io;
use crate::io::SeekFrom;
use crate::io::{DefaultProjectIo, IoTrait};
use crate::utils::MapResultExt;
use async_zip::base::read::seek::ZipFileReader;
use futures::prelude::*;
use log::trace;
use std::path::{Component, Path};

pub async fn extract_zip(
    zip_file: impl AsyncBufRead + AsyncSeek + Unpin,
    io: &DefaultProjectIo,
    dest_folder: &Path,
) -> io::Result<()> {
    extract_zip_with_progress(zip_file, io, dest_folder, |_, _, _, _| {}).await
}

pub async fn extract_zip_with_progress<F>(
    mut zip_file: impl AsyncBufRead + AsyncSeek + Unpin,
    io: &DefaultProjectIo,
    dest_folder: &Path,
    mut on_progress: F,
) -> io::Result<()>
where
    F: FnMut(usize, usize, &str, u64) + Send,
{
    // extract zip file
    zip_file.seek(SeekFrom::Start(0)).await?;

    let mut zip_reader = ZipFileReader::new(zip_file).await.err_mapped()?;
    let total_entries = zip_reader.file().entries().len();
    let mut total_uncompressed_read: u64 = 0;

    for i in 0..total_entries {
        let filename_str = {
            let entry = &zip_reader.file().entries()[i];
            let Some(filename) = entry.filename().as_str().ok() else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "path in zip file is not utf8".to_string(),
                ));
            };
            let filename = fix_path_separator(filename);
            if !is_complete_relative(filename.as_ref().as_ref()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("directory traversal detected: {filename}"),
                ));
            }
            filename.to_string()
        };

        on_progress(i, total_entries, &filename_str, total_uncompressed_read);

        let path = dest_folder.join(&filename_str);
        if filename_str.ends_with('/') {
            // if it's directory, just create directory
            io.create_dir_all(path.as_ref()).await?;
        } else {
            let mut reader = zip_reader.reader_without_entry(i).await.err_mapped()?;
            io.create_dir_all(path.parent().unwrap()).await?;
            let mut dest_file = io.create(path.as_ref()).await?;
            let copied = io::copy(&mut reader, &mut dest_file).await?;
            dest_file.flush().await?;
            total_uncompressed_read += copied;
        }

        on_progress(i + 1, total_entries, &filename_str, total_uncompressed_read);
    }

    Ok(())
}

fn fix_path_separator(p: &str) -> std::borrow::Cow<'_, str> {
    if cfg!(windows) {
        // On windows Path struct accepts both '/' and '\' as separator so we don't need to convert separator
        std::borrow::Cow::Borrowed(p)
    } else if !p.contains('\\') {
        // If the path does not contain '\\' we don't need to replace path separators
        std::borrow::Cow::Borrowed(p)
    } else {
        // The path contains '\\', we should replace with '/'
        trace!("fixing '\\' with '/' in path {p:?}");
        std::borrow::Cow::Owned(p.replace('\\', "/"))
    }
}

fn is_complete_relative(path: &Path) -> bool {
    for x in path.components() {
        match x {
            Component::Prefix(_) => return false,
            Component::RootDir => return false,
            Component::ParentDir => return false,
            Component::CurDir => {}
            Component::Normal(_) => {}
        }
    }
    true
}
