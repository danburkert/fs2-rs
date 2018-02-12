use super::FileExt;

use std::result::Result;
use std::io;
use std::ops::{Deref,DerefMut};

/// An RAII implementation of a "scoped lock" of a file.
/// When this structure is dropped (falls out of scope), the file will be unlocked.
///
/// This structure is created by the [`lock_shared_guard`], [`lock_exclusive_guard`], [`try_lock_shared_guard`], and [`try_lock_exclusive_guard`] methods on [`FileExt`].
///
/// # Examples
///
/// ```
/// use fs2::FileExt;
/// use fs2::FileLock;
/// use std::io::Write;
///
/// fn exclusive_access(file: &mut std::fs::File) {
///     if let Ok(mut locked) = file.try_lock_exclusive_guard() {
///
///         // exclusive operation, synchronized among processes
///         write!(*locked, "hello").unwrap();
///
///         // file is unlocked when "locked" guard leaves scope
///     }
/// }
/// ```
///
/// [`lock_shared_guard`]: trait.FileExt.html#tymethod.lock_shared_guard
/// [`lock_exclusive_guard`]: trait.FileExt.html#tymethod.lock_exclusive_guard
/// [`try_lock_shared_guard`]: trait.FileExt.html#tymethod.try_lock_shared_guard
/// [`try_lock_exclusive_guard`]: trait.FileExt.html#tymethod.try_lock_exclusive_guard
/// [`FileExt`]: trait.FileExt.html
#[derive(Debug)]
pub struct FileLockGuard<'a, T: FileExt + ?Sized + 'a> {
    file: &'a mut T,
}

impl<'a, T: FileExt + ?Sized + 'a> FileLockGuard<'a, T> {

    /// Create a lock guard. The file must already be locked.
    fn new(file: &mut T) -> FileLockGuard<T> {
        FileLockGuard {
            file
        }
    }
}

impl<'a, T: FileExt + ?Sized + 'a> Deref for FileLockGuard<'a, T> {
    type Target = T;

    /// Access locked file.
    fn deref(&self) -> &T {
        self.file
    }
}

impl<'a, T: FileExt + ?Sized + 'a> DerefMut for FileLockGuard<'a, T> {

    /// Mutably access locked file.
    fn deref_mut(&mut self) -> &mut T {
        self.file
    }
}

impl<'a, T: FileExt + ?Sized + 'a> Drop for FileLockGuard<'a, T> {

    /// Unlock the locked file.
    ///
    /// # Panics
    /// `drop()` panics if the unlock operation fails.
    /// Unlock should not fail on Unix, since we guarantee the file descriptor is still valid and the lock succeeded.
    /// Windows does not document what may cause unlock to fail.
    fn drop(&mut self) {
        self.file.unlock().unwrap();
    }
}

pub type FileLockResult<'a, T> = Result<FileLockGuard<'a, T>, io::Error>;

pub trait FileLock: FileExt {

    /// [`lock_shared`](#tymethod.lock_shared),
    /// then unlock when the returned `FileLockGuard` exits scope.
    fn lock_shared_guard(&mut self) -> FileLockResult<Self>;

    /// [`lock_exclusive`](#tymethod.lock_exclusive),
    /// then unlock when the returned `FileLockGuard` exits scope.
    fn lock_exclusive_guard(&mut self) -> FileLockResult<Self>;

    /// [`try_lock_shared`](#tymethod.try_lock_shared),
    /// then unlock when the returned `FileLockGuard` exits scope.
    fn try_lock_shared_guard(&mut self) -> FileLockResult<Self>;

    /// [`try_lock_exclusive`](#tymethod.try_lock_exclusive),
    /// then unlock when the returned `FileLockGuard` exits scope.
    fn try_lock_exclusive_guard(&mut self) -> FileLockResult<Self>;
}

impl<T: FileExt> FileLock for T {

    fn lock_shared_guard(&mut self) -> FileLockResult<Self> {
        self.lock_shared()?;
        Ok(FileLockGuard::new(self))
    }

    fn lock_exclusive_guard(&mut self) -> FileLockResult<Self> {
        self.lock_exclusive()?;
        Ok(FileLockGuard::new(self))
    }

    fn try_lock_shared_guard(&mut self) -> FileLockResult<Self> {
        self.try_lock_shared()?;
        Ok(FileLockGuard::new(self))
    }

    fn try_lock_exclusive_guard(&mut self) -> FileLockResult<Self> {
        self.try_lock_exclusive()?;
        Ok(FileLockGuard::new(self))
    }
}


#[cfg(test)]
mod test {

    extern crate tempdir;

    use std::fs;
    use super::*;
    use super::super::lock_contended_error;

    /// Tests guarded shared file lock operations.
    #[test]
    fn lock_shared_guard() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let mut file1 = fs::OpenOptions::new().read(true).write(true).create(true).open(&path).unwrap();
        let mut file2 = fs::OpenOptions::new().read(true).write(true).create(true).open(&path).unwrap();
        let mut file3 = fs::OpenOptions::new().read(true).write(true).create(true).open(&path).unwrap();

        // Concurrent shared access is OK, but not shared and exclusive.
        let guard1 = file1.lock_shared_guard().unwrap();
        let guard2 = file2.lock_shared_guard().unwrap();
        assert_eq!(file3.try_lock_exclusive_guard().unwrap_err().kind(),
                   lock_contended_error().kind());
        drop(guard1);
        assert_eq!(file3.try_lock_exclusive_guard().unwrap_err().kind(),
                   lock_contended_error().kind());

        // Once all shared file locks are dropped, an exclusive lock may be created;
        drop(guard2);
        file3.lock_exclusive_guard().unwrap();
    }

    /// Tests guarded exclusive file lock operations.
    #[test]
    fn lock_exclusive_guard() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let mut file1 = fs::OpenOptions::new().read(true).write(true).create(true).open(&path).unwrap();
        let mut file2 = fs::OpenOptions::new().read(true).write(true).create(true).open(&path).unwrap();

        // No other access is possible once an exclusive lock is created.
        let guard1 = file1.lock_exclusive_guard().unwrap();
        assert_eq!(file2.try_lock_exclusive_guard().unwrap_err().kind(),
                   lock_contended_error().kind());
        assert_eq!(file2.try_lock_shared_guard().unwrap_err().kind(),
                   lock_contended_error().kind());

        // Once the exclusive lock is dropped, the second file is able to create a lock.
        drop(guard1);
        file2.lock_exclusive_guard().unwrap();
    }
}
