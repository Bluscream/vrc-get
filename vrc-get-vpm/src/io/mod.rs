use std::ffi::OsString;
use std::future::Future;
use std::path::Path;

pub(crate) use futures::Stream;
pub(crate) use futures::io::{
    AsyncRead, AsyncSeek, AsyncWrite, BufReader, Error, ErrorKind, Result, copy, empty, sink,
};
pub(crate) use std::io::SeekFrom;

#[cfg(feature = "tokio")]
mod tokio;
#[cfg(feature = "tokio")]
pub use tokio::DefaultEnvironmentIo;
#[cfg(feature = "tokio")]
pub use tokio::DefaultProjectIo;
#[cfg(feature = "tokio")]
pub use tokio::DirEntry as TokioDirEntry;
#[cfg(feature = "tokio")]
pub use tokio::File as TokioFile;

#[cfg(not(feature = "tokio"))]
pub use no_tokio::*;
#[cfg(not(feature = "tokio"))]
mod no_tokio {
    use crate::io;
    use crate::io::{FileStream, FileType, IoTrait, Metadata};
    use futures::{Stream, TryFutureExt};
    use log::debug;
    #[cfg(feature = "vrc-get-litedb")]
    use std::ffi::OsStr;
    use std::ffi::OsString;
    use std::fs;
    use std::io::Write;
    use std::mem::forget;
    use std::path::Path;
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[derive(Debug, Clone)]
    pub struct DefaultEnvironmentIo {
        root: Box<Path>,
    }

    impl DefaultEnvironmentIo {
        #[inline]
        pub fn resolve_impl(&self, path: &Path) -> PathBuf {
            self.root.join(path)
        }

        pub fn new_project_io(&self, path: &Path) -> DefaultProjectIo {
            DefaultProjectIo::new(path.into())
        }
    }

    impl TokioIoTraitImpl for DefaultEnvironmentIo {
        #[inline]
        fn resolve(&self, path: &Path) -> io::Result<PathBuf> {
            Ok(self.root.join(path))
        }
    }

    #[derive(Debug)]
    pub struct DefaultProjectIo {
        root: Box<Path>,
    }

    impl DefaultProjectIo {
        pub fn new(root: Box<Path>) -> Self {
            Self { root }
        }

        pub fn find_project_parent(path_buf: PathBuf) -> io::Result<Self> {
            Self::find_unity_project_path(path_buf).map(Self::new)
        }

        fn find_unity_project_path(mut candidate: PathBuf) -> io::Result<Box<Path>> {
            loop {
                candidate.push("Packages");
                candidate.push("vpm-manifest.json");

                if candidate.exists() {
                    debug!("vpm-manifest.json found at {}", candidate.display());
                    // if there's vpm-manifest.json, it's a project path
                    candidate.pop();
                    candidate.pop();
                    return Ok(candidate.into_boxed_path());
                }

                // replace vpm-manifest.json -> manifest.json
                candidate.pop();
                candidate.push("manifest.json");

                if candidate.exists() {
                    debug!("manifest.json found at {}", candidate.display());
                    // if there's manifest.json (which is manifest of UPM), it's a project path
                    candidate.pop();
                    candidate.pop();
                    return Ok(candidate.into_boxed_path());
                }

                // remove Packages/manifest.json
                candidate.pop();
                candidate.pop();

                debug!("Unity Project not found on {}", candidate.display());

                // go to parent dir
                if !candidate.pop() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "Unity project Not Found",
                    ));
                }
            }
        }

        #[inline]
        pub fn location(&self) -> &Path {
            &self.root
        }
    }

    impl TokioIoTraitImpl for DefaultProjectIo {
        #[inline]
        fn resolve(&self, path: &Path) -> io::Result<PathBuf> {
            if path.is_absolute() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "absolute path is not allowed",
                ));
            }
            Ok(self.root.join(path))
        }
    }

    trait TokioIoTraitImpl {
        fn resolve(&self, path: &Path) -> io::Result<PathBuf>;
    }

    impl<T: TokioIoTraitImpl + Sync> IoTrait for T {
        async fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            fs::create_dir_all(self.resolve(path)?)
        }

        async fn write(&self, path: &Path, content: &[u8]) -> io::Result<()> {
            fs::write(self.resolve(path)?, content)
        }

        async fn write_sync(&self, path: &Path, content: &[u8]) -> io::Result<()> {
            let path = self.resolve(path)?;
            let mut file = fs::File::create(&path)?;
            file.write_all(content)?;
            file.flush()?;
            file.sync_data()?;
            Ok(())
        }

        async fn write_atomic(&self, path: &Path, content: &[u8]) -> io::Result<()> {
            let path = self.resolve(path)?;
            let (temp_path, mut temp) = make_temp(&path)?;
            let remove_on_drop = RemoveOnDrop { path: &temp_path };
            temp.write_all(content)?;
            temp.flush()?;
            temp.sync_data()?;
            drop(temp);
            forget(remove_on_drop);
            fs::rename(&temp_path, path)?;
            return Ok(());

            fn make_temp(path: &Path) -> io::Result<(PathBuf, fs::File)> {
                let suffix = ".temp.";
                let Some(dir) = path.parent() else {
                    return Err(io::Error::new(io::ErrorKind::IsADirectory, "RootDir"));
                };
                let file_name = path.file_name().unwrap();
                for i in 0u32.. {
                    let int_len = (i.checked_ilog10().unwrap_or(0) + 1) as usize;
                    let mut name_buf =
                        OsString::with_capacity(file_name.len() + suffix.len() + int_len);
                    name_buf.push(file_name);
                    name_buf.push(suffix);
                    name_buf.push(format!("{i}"));

                    let temp_path = dir.join(name_buf);
                    match fs::File::create_new(&temp_path) {
                        Ok(f) => return Ok((temp_path, f)),
                        Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                        Err(e) => return Err(e),
                    }
                }
                unreachable!("almost infinite loop")
            }

            struct RemoveOnDrop<'a> {
                path: &'a Path,
            }

            impl<'a> Drop for RemoveOnDrop<'a> {
                fn drop(&mut self) {
                    // ignore errors
                    std::fs::remove_file(self.path).ok();
                }
            }
        }

        async fn remove_file(&self, path: &Path) -> io::Result<()> {
            fs::remove_file(self.resolve(path)?)
        }

        async fn remove_dir(&self, path: &Path) -> io::Result<()> {
            fs::remove_dir(self.resolve(path)?)
        }

        async fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
            fs::remove_dir_all(self.resolve(path)?)
        }

        async fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            fs::rename(self.resolve(from)?, self.resolve(to)?)
        }

        async fn metadata(&self, path: &Path) -> io::Result<Metadata> {
            fs::metadata(self.resolve(path)?).map(Into::into)
        }

        type DirEntry = DirEntry;
        type ReadDirStream = ReadDir;

        async fn read_dir(&self, path: &Path) -> io::Result<Self::ReadDirStream> {
            Ok(ReadDir::new(fs::read_dir(self.resolve(path)?)?))
        }

        type FileStream = futures::io::AllowStdIo<fs::File>;

        async fn create_new(&self, path: &Path) -> io::Result<Self::FileStream> {
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .read(true)
                .open(self.resolve(path)?)
                .map(|file| futures::io::AllowStdIo::new(file))
        }

        async fn create(&self, path: &Path) -> io::Result<Self::FileStream> {
            fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .read(true)
                .open(self.resolve(path)?)
                .map(|file| futures::io::AllowStdIo::new(file))
        }

        async fn open(&self, path: &Path) -> io::Result<Self::FileStream> {
            Ok(futures::io::AllowStdIo::new(fs::File::open(
                self.resolve(path)?,
            )?))
        }
    }

    impl FileStream for futures::io::AllowStdIo<fs::File> {}

    pub type TokioFile = futures::io::AllowStdIo<fs::File>;
    pub type TokioDirEntry = DirEntry;

    pub struct ReadDir {
        inner: fs::ReadDir,
    }

    impl ReadDir {
        pub fn new(inner: fs::ReadDir) -> Self {
            Self { inner }
        }
    }

    impl Stream for ReadDir {
        type Item = io::Result<DirEntry>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            panic!("C# implemented")
        }
    }

    pub struct DirEntry {
        inner: fs::DirEntry,
    }

    impl DirEntry {
        pub fn new(inner: fs::DirEntry) -> Self {
            Self { inner }
        }
    }

    impl super::DirEntry for DirEntry {
        fn file_name(&self) -> OsString {
            self.inner.file_name()
        }

        async fn file_type(&self) -> io::Result<FileType> {
            self.inner.file_type().map(Into::into)
        }

        async fn metadata(&self) -> io::Result<Metadata> {
            self.inner.metadata().map(Into::into)
        }
    }
}

pub trait IoTrait: Sync {
    fn create_dir_all(&self, path: &Path) -> impl Future<Output = Result<()>> + Send;
    fn write(&self, path: &Path, content: &[u8]) -> impl Future<Output = Result<()>> + Send;
    fn write_sync(&self, path: &Path, content: &[u8]) -> impl Future<Output = Result<()>> + Send;
    /// Atomically writes file.
    /// This works as:
    /// 1. Create new file with different name
    /// 2. Write contents and flush data
    /// 3. Rename it to the filename
    fn write_atomic(&self, path: &Path, content: &[u8]) -> impl Future<Output = Result<()>> + Send;
    fn remove_file(&self, path: &Path) -> impl Future<Output = Result<()>> + Send;
    fn remove_dir(&self, path: &Path) -> impl Future<Output = Result<()>> + Send;
    fn remove_dir_all(&self, path: &Path) -> impl Future<Output = Result<()>> + Send;
    fn rename(&self, from: &Path, to: &Path) -> impl Future<Output = Result<()>> + Send;
    fn metadata(&self, path: &Path) -> impl Future<Output = Result<Metadata>> + Send;

    type DirEntry: DirEntry;
    type ReadDirStream: Stream<Item = Result<Self::DirEntry>> + Unpin + Send;

    fn is_file(&self, path: &Path) -> impl Future<Output = bool> + Send {
        async {
            self.metadata(path)
                .await
                .map(|x| x.is_file())
                .unwrap_or(false)
        }
    }

    fn is_dir(&self, path: &Path) -> impl Future<Output = bool> + Send {
        async {
            self.metadata(path)
                .await
                .map(|x| x.is_dir())
                .unwrap_or(false)
        }
    }

    fn read_dir(&self, path: &Path) -> impl Future<Output = Result<Self::ReadDirStream>> + Send;

    type FileStream: FileStream;

    fn create_new(&self, path: &Path) -> impl Future<Output = Result<Self::FileStream>> + Send;
    fn create(&self, path: &Path) -> impl Future<Output = Result<Self::FileStream>> + Send;
    fn open(&self, path: &Path) -> impl Future<Output = Result<Self::FileStream>> + Send;
}

pub trait FileStream: AsyncRead + AsyncWrite + AsyncSeek + Unpin + Send {}

#[derive(Debug, Copy, Clone)]
pub struct FileType {
    is_file: bool,
    is_dir: bool,
}

impl FileType {
    pub fn file() -> Self {
        Self {
            is_file: true,
            is_dir: false,
        }
    }

    pub fn dir() -> Self {
        Self {
            is_file: false,
            is_dir: true,
        }
    }

    pub fn is_file(&self) -> bool {
        self.is_file
    }

    pub fn is_dir(&self) -> bool {
        self.is_dir
    }
}

impl From<std::fs::FileType> for FileType {
    fn from(value: std::fs::FileType) -> Self {
        Self {
            is_dir: value.is_dir(),
            is_file: value.is_file(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Metadata {
    file_type: FileType,
}

impl Metadata {
    pub fn file() -> Self {
        Self {
            file_type: FileType::file(),
        }
    }

    pub fn dir() -> Self {
        Self {
            file_type: FileType::dir(),
        }
    }

    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    pub fn is_file(&self) -> bool {
        self.file_type.is_file
    }

    pub fn is_dir(&self) -> bool {
        self.file_type.is_dir
    }
}

impl From<std::fs::Metadata> for Metadata {
    fn from(value: std::fs::Metadata) -> Self {
        Self {
            file_type: value.file_type().into(),
        }
    }
}

/*
#[derive(Debug)]
pub struct ExitStatus {
    inner: ExitStatusEnum,
}

#[derive(Debug)]
enum ExitStatusEnum {
    Std(std::process::ExitStatus),
    Custom { success: bool },
}

impl ExitStatus {
    pub fn new(success: bool) -> Self {
        Self {
            inner: ExitStatusEnum::Custom { success },
        }
    }

    pub fn success(&self) -> bool {
        match self.inner {
            ExitStatusEnum::Std(std) => std.success(),
            ExitStatusEnum::Custom { success, .. } => success,
        }
    }
}

impl From<std::process::ExitStatus> for ExitStatus {
    fn from(value: std::process::ExitStatus) -> Self {
        Self {
            inner: ExitStatusEnum::Std(value),
        }
    }
}

impl std::fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            ExitStatusEnum::Std(std) => std::fmt::Display::fmt(std, f),
            ExitStatusEnum::Custom { success: true, .. } => f.write_str("exits successfully"),
            ExitStatusEnum::Custom { success: false, .. } => f.write_str("exits non successfully"),
        }
    }
}
 */

pub trait DirEntry {
    fn file_name(&self) -> OsString;
    fn file_type(&self) -> impl Future<Output = Result<FileType>> + Send;
    fn metadata(&self) -> impl Future<Output = Result<Metadata>> + Send;
}
