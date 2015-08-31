#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::{
    duplicate,
    lock_exclusive,
    lock_exclusive_nonblock,
    lock_shared,
    lock_shared_nonblock,
    unlock,
};

use std::fs::File;
use std::io::Result;

/// Extension trait for `File`s.
///
/// ## Notes on File Locks
///
/// File locks are implemented with
/// [`flock(2)`](http://man7.org/linux/man-pages/man2/flock.2.html) on Unix and
/// [`LockFile`](https://msdn.microsoft.com/en-us/library/windows/desktop/aa365202(v=vs.85).aspx)
/// on Windows.
///
/// File-range locks are not provided, since suitable APIs only exist for Linux and Windows. Posix
/// file locks [`fctnl(2)`](http://man7.org/linux/man-pages/man2/fcntl.2.html)) are
/// [broken](https://lwn.net/Articles/586904/) in multi-threaded applications, applications which
/// call libraries that open files, and applications which use RAII-style file resource wrappers
/// (i.e. Rust applications).
///
/// On Unix, the lock is advisory; given suitable permissions on a file, a process is free to
/// ignore the lock and perform I/O on the file.
///
/// On Windows, the lock is mandatory.
///
/// On Linux, the file lock will not interact with
/// [`fcntl(2)`](http://man7.org/linux/man-pages/man2/fcntl.2.html) file-range locks.
///
/// On Unix, converting a lock (shared to exclusive, or vice versa) is not guaranteed to be atomic:
/// the existing lock is first removed, and then a new lock is established. Between these two
/// steps, a pending lock request by another process may be granted, with the result that the
/// conversion either blocks, or fails if a non-blocking operation was specified.
///
/// On Windows, the locked file must be opened with read or write permissions.
///
/// When a locked `File` is dropped (or the last duplicate, if the `File` has been duplicated), the
/// lock will be unlocked, however on Windows this is not guaranteed to happen immediately.
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

    /// Locks the file for shared usage, or returns `ErrorKind::WouldBlock` if the file is currently
    /// locked exclusively.
    fn lock_shared_nonblock(&self) -> Result<()>;

    /// Locks the file for shared usage, or returns `ErrorKind::WouldBlock` if the file is currently
    /// locked.
    fn lock_exclusive_nonblock(&self) -> Result<()>;

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
    fn lock_shared_nonblock(&self) -> Result<()> {
        lock_shared_nonblock(self)
    }
    fn lock_exclusive_nonblock(&self) -> Result<()> {
        lock_exclusive_nonblock(self)
    }
    fn unlock(&self) -> Result<()> {
        unlock(self)
    }
}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use std::fs;
    use super::FileExt;
    use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};

    /// Tests file duplication.
    #[test]
    fn duplicate() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let mut file1 = fs::OpenOptions::new().read(true).write(true).create(true).open(&path).unwrap();
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
        let file1 = fs::OpenOptions::new().create(true).open(&path).unwrap();
        let file2 = fs::OpenOptions::new().create(true).open(&path).unwrap();
        let file3 = fs::OpenOptions::new().create(true).open(&path).unwrap();

        // Concurrent shared access is OK, but not shared and exclusive.
        file1.lock_shared().unwrap();
        file2.lock_shared().unwrap();
        assert_eq!(file3.lock_exclusive_nonblock().unwrap_err().kind(), ErrorKind::WouldBlock);
        file1.unlock().unwrap();
        assert_eq!(file3.lock_exclusive_nonblock().unwrap_err().kind(), ErrorKind::WouldBlock);

        // Once all shared file locks are dropped, an exclusive lock may be created;
        file2.unlock().unwrap();
        file3.lock_exclusive().unwrap();
    }

    /// Tests exclusive file lock operations.
    #[test]
    fn lock_exclusive() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().create(true).open(&path).unwrap();
        let file2 = fs::OpenOptions::new().create(true).open(&path).unwrap();

        // No other access is possible once an exclusive lock is created.
        file1.lock_exclusive().unwrap();
        assert_eq!(file2.lock_exclusive_nonblock().unwrap_err().kind(), ErrorKind::WouldBlock);
        assert_eq!(file2.lock_shared_nonblock().unwrap_err().kind(), ErrorKind::WouldBlock);

        // Once the exclusive lock is dropped, the second file is able to create a lock.
        file1.unlock().unwrap();
        file2.lock_exclusive().unwrap();
    }

    /// Tests that opening locks on a file descriptor will replace any existing locks on the file.
    #[test]
    fn lock_replace() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().create(true).open(&path).unwrap();
        let file2 = fs::OpenOptions::new().create(true).open(&path).unwrap();

        // Creating a shared lock will drop an exclusive lock.
        file1.lock_exclusive().unwrap();
        file1.lock_shared().unwrap();
        file2.lock_shared().unwrap();

        // Attempting to replace a shared lock with an exclusive lock will fail with multiple lock
        // holders, and remove the original shared lock.
        assert_eq!(file2.lock_exclusive_nonblock().unwrap_err().kind(), ErrorKind::WouldBlock);
        file1.lock_shared().unwrap();
    }

    /// Tests that locks are shared among duplicate file descriptors.
    #[test]
    fn lock_duplicate() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().create(true).open(&path).unwrap();
        let file2 = file1.duplicate().unwrap();
        let file3 = fs::OpenOptions::new().create(true).open(&path).unwrap();

        // Create a lock through fd1, then replace it through fd2.
        file1.lock_shared().unwrap();
        file2.lock_exclusive().unwrap();
        assert_eq!(file3.lock_shared_nonblock().unwrap_err().kind(), ErrorKind::WouldBlock);

        // Either of the file descriptors should be able to unlock.
        file1.unlock().unwrap();
        file3.lock_shared().unwrap();
    }

    /// Tests that a lock is cleaned up after the last duplicated file descriptor is closed.
    #[test]
    fn lock_cleanup() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().create(true).open(&path).unwrap();
        let file2 = file1.duplicate().unwrap();
        let file3 = fs::OpenOptions::new().create(true).open(&path).unwrap();

        // Create a lock through fd1, then replace it through fd2.
        file1.lock_shared().unwrap();
        file2.lock_exclusive().unwrap();
        assert_eq!(file3.lock_shared_nonblock().unwrap_err().kind(), ErrorKind::WouldBlock);

        // Either of the file descriptors should be able to unlock.
        file1.unlock().unwrap();
        file3.lock_shared().unwrap();
    }
}
