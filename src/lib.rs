#![cfg_attr(test, feature(test))]
#![deny(warnings)]

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::{
    duplicate,
    lock_error,
    lock_exclusive,
    lock_shared,
    try_lock_exclusive,
    try_lock_shared,
    unlock,
};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::{
    duplicate,
    lock_error,
    lock_exclusive,
    lock_shared,
    try_lock_exclusive,
    try_lock_shared,
    unlock,
};

use std::fs::File;
use std::io::{Error, Result};

/// Extension trait for `File` providing duplication and locking methods.
///
/// ## Notes on File Locks
///
/// This library provides whole-file locks in both shared (read) and exclusive (read-write)
/// varieties.
///
/// File locks are a cross-platform hazard since the file lock APIs exposed by operating system
/// kernels vary in subtle and not-so-subtle ways.
///
/// The API exposed by this library can be safely used across platforms as long as the following
/// rules are followed:
///
///   * Multiple locks should not be created on an individual `File` instance concurrently.
///   * Duplicated files should not be locked without great care.
///   * Files to be locked should be opened with at least read or write permissions.
///   * File locks may only be relied upon to be advisory.
///
/// See the tests in `lib.rs` for cross-platform lock behavior that may be relied upon; see the
/// tests in `unix.rs` and `windows.rs` for examples of platform-specific behavior. File locks are
/// implemented with
/// [`flock(2)`](http://man7.org/linux/man-pages/man2/flock.2.html) on Unix and
/// [`LockFile`](https://msdn.microsoft.com/en-us/library/windows/desktop/aa365202(v=vs.85).aspx)
/// on Windows.
pub trait FileExt {

    /// Returns a duplicate instance of the file.
    ///
    /// The returned file will share the same file position as the original file.
    ///
    /// # Notes
    ///
    /// This is implemented with [`dup(2)`](http://man7.org/linux/man-pages/man2/dup.2.html)
    /// on Unix and
    /// [`DuplicateHandle`](https://msdn.microsoft.com/en-us/library/windows/desktop/ms724251(v=vs.85).aspx)
    /// on Windows.
    fn duplicate(&self) -> Result<File>;

    /// Locks the file for shared usage, blocking if the file is currently locked exclusively.
    fn lock_shared(&self) -> Result<()>;

    /// Locks the file for exclusive usage, blocking if the file is currently locked.
    fn lock_exclusive(&self) -> Result<()>;

    /// Locks the file for shared usage, or returns a an error if the file is currently locked
    /// (see `lock_contended_error`).
    fn try_lock_shared(&self) -> Result<()>;

    /// Locks the file for shared usage, or returns a an error if the file is currently locked
    /// (see `lock_contended_error`).
    fn try_lock_exclusive(&self) -> Result<()>;

    /// Unlocks the file.
    fn unlock(&self) -> Result<()>;
}

impl FileExt for File {
    fn duplicate(&self) -> Result<File> {
        duplicate(self)
    }
    fn lock_shared(&self) -> Result<()> {
        lock_shared(self)
    }
    fn lock_exclusive(&self) -> Result<()> {
        lock_exclusive(self)
    }
    fn try_lock_shared(&self) -> Result<()> {
        try_lock_shared(self)
    }
    fn try_lock_exclusive(&self) -> Result<()> {
        try_lock_exclusive(self)
    }
    fn unlock(&self) -> Result<()> {
        unlock(self)
    }
}

/// Returns the error that a call to a try lock method on a contended file will return.
pub fn lock_contended_error() -> Error {
    lock_error()
}

#[cfg(test)]
mod test {

    extern crate tempdir;
    extern crate test;

    use std::fs;
    use super::{lock_contended_error, FileExt};
    use std::io::{Read, Seek, SeekFrom, Write};

    /// Tests file duplication.
    #[test]
    fn duplicate() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let mut file1 =
            fs::OpenOptions::new().read(true).write(true).create(true).open(&path).unwrap();
        let mut file2 = file1.duplicate().unwrap();

        // Write into the first file and then drop it.
        file1.write_all(b"foo").unwrap();
        drop(file1);

        let mut buf = vec![];

        // Read from the second file; since the position is shared it will already be at EOF.
        file2.read_to_end(&mut buf).unwrap();
        assert_eq!(0, buf.len());

        // Rewind and read.
        file2.seek(SeekFrom::Start(0)).unwrap();
        file2.read_to_end(&mut buf).unwrap();
        assert_eq!(&buf, &b"foo");
    }

    /// Tests shared file lock operations.
    #[test]
    fn lock_shared() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().create(true).read(true).open(&path).unwrap();
        let file2 = fs::OpenOptions::new().create(true).read(true).open(&path).unwrap();
        let file3 = fs::OpenOptions::new().create(true).read(true).open(&path).unwrap();

        // Concurrent shared access is OK, but not shared and exclusive.
        file1.lock_shared().unwrap();
        file2.lock_shared().unwrap();
        assert_eq!(file3.try_lock_exclusive().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());
        file1.unlock().unwrap();
        assert_eq!(file3.try_lock_exclusive().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());

        // Once all shared file locks are dropped, an exclusive lock may be created;
        file2.unlock().unwrap();
        file3.lock_exclusive().unwrap();
    }

    /// Tests exclusive file lock operations.
    #[test]
    fn lock_exclusive() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();
        let file2 = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();

        // No other access is possible once an exclusive lock is created.
        file1.lock_exclusive().unwrap();
        assert_eq!(file2.try_lock_exclusive().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());
        assert_eq!(file2.try_lock_shared().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());

        // Once the exclusive lock is dropped, the second file is able to create a lock.
        file1.unlock().unwrap();
        file2.lock_exclusive().unwrap();
    }

    /// Tests that a lock is released after the file that owns it is dropped.
    #[test]
    fn lock_cleanup() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();
        let file2 = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();

        file1.lock_exclusive().unwrap();
        assert_eq!(file2.try_lock_shared().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());

        // Drop file1; the lock should be released.
        drop(file1);
        file2.lock_shared().unwrap();
    }

    #[bench]
    fn bench_duplicate(b: &mut test::Bencher) {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();

        b.iter(|| test::black_box(file.duplicate().unwrap()));
    }

    #[bench]
    fn bench_lock_unlock(b: &mut test::Bencher) {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();

        b.iter(|| {
            file.lock_exclusive().unwrap();
            file.unlock().unwrap();
        });
    }
}
