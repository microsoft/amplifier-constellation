//! Bounded snapshot reads after the enrolled path and owner-policy preflight.
//!
//! The opened file is retained for the whole read. Windows sharing is explicit:
//! data publication may replace the pathname while this handle retains its old
//! snapshot. This does not protect against in-place writes or hostile changes to
//! parent paths by another process with the same owner's authority.

use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    path::Path,
};

pub const MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub enum ReadError {
    TooLarge,
    Io(io::Error),
}

impl From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn limit(value: &serde_json::Value) -> Option<usize> {
    value
        .as_u64()
        .filter(|value| (1..=MAX_BYTES as u64).contains(value))
        .map(|value| value as usize)
}

fn open(path: &Path) -> io::Result<Option<File>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Never follow a substituted final symlink or block opening a FIFO.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Data snapshot must be an ordinary file",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Data snapshot cannot be a reparse point",
            ));
        }
    }
    Ok(Some(file))
}

fn read_opened(file: &mut File, max_bytes: usize) -> Result<Vec<u8>, ReadError> {
    if !(1..=MAX_BYTES).contains(&max_bytes) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid read limit").into());
    }
    // A fixed buffer avoids read_to_end's allocation growth beyond the bound.
    // Read one extra byte to distinguish exact fit from an oversized snapshot.
    let mut bytes = vec![0; max_bytes + 1];
    let mut length = 0;
    while length < bytes.len() {
        match file.read(&mut bytes[length..]) {
            Ok(0) => break,
            Ok(read) => length += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    if length > max_bytes {
        return Err(ReadError::TooLarge);
    }
    bytes.truncate(length);
    Ok(bytes)
}

pub fn read(path: &Path, max_bytes: usize) -> Result<Option<Vec<u8>>, ReadError> {
    // Only the actual open can establish absence. Metadata/read errors after
    // opening never become absence; the caller rechecks enrolled parent policy.
    let Some(mut file) = open(path)? else {
        return Ok(None);
    };
    read_opened(&mut file, max_bytes).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use amplifier_native_platform::{private_create_dir, private_write_data};
    use serde_json::json;
    use std::fs;

    fn temporary() -> tempfile::TempDir {
        match std::env::var_os("CONSTELLATION_NATIVE_TEST_ROOT") {
            Some(root) => tempfile::tempdir_in(root).unwrap(),
            None => tempfile::tempdir().unwrap(),
        }
    }

    #[test]
    fn limit_requires_a_bounded_positive_json_integer() {
        for value in [
            json!(null),
            json!(false),
            json!(true),
            json!(0),
            json!(-1),
            json!(1.0),
            json!(1.5),
            json!("1"),
            json!([]),
            json!({}),
            json!(MAX_BYTES + 1),
            json!(u64::MAX),
        ] {
            assert_eq!(limit(&value), None, "{value}");
        }
        assert_eq!(limit(&json!(1)), Some(1));
        assert_eq!(limit(&json!(MAX_BYTES)), Some(MAX_BYTES));
    }

    #[test]
    fn absent_empty_exact_and_oversize_are_distinct() {
        let directory = temporary();
        let path = directory.path().join("journal");
        assert_eq!(read(&path, 3).unwrap(), None);
        fs::write(&path, []).unwrap();
        assert_eq!(read(&path, 3).unwrap(), Some(vec![]));
        fs::write(&path, b"abc").unwrap();
        assert_eq!(read(&path, 3).unwrap(), Some(b"abc".to_vec()));
        fs::write(&path, b"abcd").unwrap();
        assert!(matches!(read(&path, 3), Err(ReadError::TooLarge)));
        assert_eq!(fs::read(&path).unwrap(), b"abcd");
    }

    #[test]
    fn oversized_read_stops_after_one_extra_byte() {
        let directory = temporary();
        let path = directory.path().join("journal");
        fs::write(&path, [7; 128]).unwrap();
        let mut file = open(&path).unwrap().unwrap();
        assert!(matches!(
            read_opened(&mut file, 3),
            Err(ReadError::TooLarge)
        ));
        use std::io::Seek;
        assert_eq!(file.stream_position().unwrap(), 4);
    }

    #[test]
    fn production_handle_retains_old_snapshot_after_data_publication() {
        let directory = temporary();
        let root = directory.path().join("private");
        private_create_dir(&root).unwrap();
        let path = root.join("journal");
        private_write_data(&path, b"old snapshot").unwrap();
        let mut old = open(&path).unwrap().unwrap();
        #[cfg(windows)]
        assert!(amplifier_native_platform::private_write(&path, b"strict").is_err());
        private_write_data(&path, b"new snapshot").unwrap();
        assert_eq!(read_opened(&mut old, 64).unwrap(), b"old snapshot");
        assert_eq!(read(&path, 64).unwrap(), Some(b"new snapshot".to_vec()));
    }

    #[test]
    fn directory_is_an_error_not_absence() {
        let directory = temporary();
        assert!(matches!(read(directory.path(), 3), Err(ReadError::Io(_))));
    }

    #[cfg(unix)]
    #[test]
    fn substituted_symlink_is_never_followed_even_when_dangling() {
        use std::os::unix::fs::symlink;
        let directory = temporary();
        let target = directory.path().join("target");
        let link = directory.path().join("journal");
        fs::write(&target, b"private").unwrap();
        symlink(&target, &link).unwrap();
        assert!(matches!(read(&link, 64), Err(ReadError::Io(_))));
        fs::remove_file(target).unwrap();
        assert!(matches!(read(&link, 64), Err(ReadError::Io(_))));
    }

    #[cfg(unix)]
    #[test]
    fn substituted_fifo_is_rejected_without_waiting_for_a_writer() {
        use std::os::unix::ffi::OsStrExt;
        let directory = temporary();
        let path = directory.path().join("journal");
        let native = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(native.as_ptr(), 0o600) }, 0);
        assert!(matches!(read(&path, 64), Err(ReadError::Io(_))));
    }

    #[cfg(windows)]
    #[test]
    fn denied_sharing_remains_an_error_for_reader_and_publisher() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
        let directory = temporary();
        let root = directory.path().join("private");
        private_create_dir(&root).unwrap();
        let path = root.join("journal");
        private_write_data(&path, b"original").unwrap();
        let denied_delete = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(&path)
            .unwrap();
        assert!(private_write_data(&path, b"unpublished").is_err());
        assert_eq!(read(&path, 64).unwrap(), Some(b"original".to_vec()));
        drop(denied_delete);
        let exclusive = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        assert!(matches!(read(&path, 64), Err(ReadError::Io(_))));
        drop(exclusive);
        private_write_data(&path, b"replacement").unwrap();
        assert_eq!(read(&path, 64).unwrap(), Some(b"replacement".to_vec()));
    }
}
