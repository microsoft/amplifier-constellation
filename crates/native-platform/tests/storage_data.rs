use amplifier_native_platform::{private_create, private_write, private_write_data};
use std::fs;

#[test]
fn data_publication_preserves_create_only_and_directory_boundaries() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("journal.json");
    private_write_data(&path, b"first").unwrap();
    private_write_data(&path, b"next").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"next");
    assert!(private_create(&path, b"not allowed").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"next");
    let directory = home.path().join("directory");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("retained"), b"evidence").unwrap();
    assert!(private_write_data(&directory, b"not a file").is_err());
    assert_eq!(fs::read(directory.join("retained")).unwrap(), b"evidence");
    // Existing callers still use their original replacement primitive.
    private_write(&path, b"original API").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"original API");
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        fs::{File, OpenOptions},
        io::Read,
        os::windows::{ffi::OsStrExt, fs::OpenOptionsExt},
        sync::{
            Arc, Barrier,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    #[test]
    fn an_open_reader_keeps_old_bytes_while_new_readers_see_publication() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("journal.json");
        private_write_data(&path, b"original").unwrap();
        let mut reader = File::open(&path).unwrap(); // Rust grants delete sharing.
        assert!(private_write(&path, b"strict API stays strict").is_err());
        private_write_data(&path, b"replacement").unwrap();
        let mut original = Vec::new();
        reader.read_to_end(&mut original).unwrap();
        assert_eq!(original, b"original");
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
    }

    #[test]
    fn denied_delete_sharing_is_an_error_without_losing_original() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("journal.json");
        private_write_data(&path, b"original").unwrap();
        let reader = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(&path)
            .unwrap();
        assert!(private_write_data(&path, b"rejected").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        drop(reader);
        private_write_data(&path, b"after close").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"after close");
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 1);
    }

    #[test]
    fn concurrent_reads_never_observe_absence_or_partial_data() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("journal.json");
        private_write_data(&path, &[0_u8; 16384]).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(Barrier::new(3));
        let mut readers = Vec::new();
        for _ in 0..2 {
            let (path, stop, ready) = (path.clone(), stop.clone(), ready.clone());
            readers.push(thread::spawn(move || -> anyhow::Result<usize> {
                let first = fs::read(&path);
                ready.wait();
                first?;
                let mut count = 0;
                while !stop.load(Ordering::Acquire) {
                    let bytes = fs::read(&path)?;
                    anyhow::ensure!(bytes.len() == 16384 && bytes.iter().all(|b| *b == bytes[0]));
                    count += 1;
                }
                Ok(count)
            }));
        }
        ready.wait();
        let result = (1..=100).try_for_each(|i| private_write_data(&path, &[i; 16384]));
        stop.store(true, Ordering::Release);
        let reads: usize = readers
            .into_iter()
            .map(|r| r.join().unwrap().unwrap())
            .sum();
        result.unwrap();
        assert!(reads > 0);
        assert_eq!(fs::read(path).unwrap(), [100; 16384]);
    }

    #[test]
    fn utf16_alignment_long_paths_and_readonly_refusal_are_preserved() {
        let home = tempfile::tempdir().unwrap();
        let mut folder = home.path().to_path_buf();
        while folder.as_os_str().encode_wide().count() < 340 {
            folder.push("journal-café-雪-0123456789");
        }
        fs::create_dir_all(&folder).unwrap();
        for length in 1..=8 {
            let path = folder.join(format!("{}-😀.json", "x".repeat(length)));
            private_write_data(&path, b"original").unwrap();
            let reader = OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
                .open(&path)
                .unwrap();
            private_write_data(&path, b"next").unwrap();
            drop(reader);
            assert_eq!(fs::read(&path).unwrap(), b"next");
            let permissions = fs::metadata(&path).unwrap().permissions();
            let mut readonly = permissions.clone();
            readonly.set_readonly(true);
            fs::set_permissions(&path, readonly).unwrap();
            let rejected = private_write_data(&path, b"must not override");
            fs::set_permissions(&path, permissions).unwrap();
            assert!(rejected.is_err());
            assert_eq!(fs::read(&path).unwrap(), b"next");
        }
        assert_eq!(fs::read_dir(folder).unwrap().count(), 8);
    }
}
