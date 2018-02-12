use super::FileExt;

use std::result::Result;
use std::io;
use std::ops::{Deref,DerefMut};

/// An RAII implementation of a "scoped lock" of a file.
/// When this structure is dropped (falls out of scope), the file will be unlocked.
///
/// This structure is created by the [`lock_shared_guard`], [`lock_exclusive_guard`], [`try_lock_shared_guard`], and [`try_lock_exclusive_guard`] methods on [`FileExt`].

/// # Examples
///
/// ```
/// use fs2::FileExt;
/// use std::io::Write;
///
/// fn exclusive_access(file: std::fs::File) {
///     if let Ok(mut locked) = file.lock_exclusive_guard() {
///
///         // exclusive operation, synchronized among processes
///         write!(*locked, "hello").unwrap();
///
///         // file is unlocked when "locked" guard leaves scope
///     }
/// }
/// ```
///
/// ```compile_fail
/// use fs2::FileExt;
///
/// fn uh_oh(file: std::fs::File) {
///     let locked = file.lock_exclusive_guard().unwrap();
///
///     // compile error:
///     // cannot close file while lock guard is open
///     drop(file);
/// }
/// ```
///
/// [`lock_shared_guard`]: trait.FileExt.html#tymethod.lock_shared_guard
/// [`lock_exclusive_guard`]: trait.FileExt.html#tymethod.lock_exclusive_guard
/// [`try_lock_shared_guard`]: trait.FileExt.html#tymethod.try_lock_shared_guard
/// [`try_lock_exclusive_guard`]: trait.FileExt.html#tymethod.try_lock_exclusive_guard
/// [`FileExt`]: trait.FileExt.html
#[derive(Debug)]
pub struct FileLockGuard<T: FileExt> {
    file: T
}

impl<T: FileExt> FileLockGuard<T> {
    fn new(file: T) -> FileLockGuard<T> {
        FileLockGuard {
            file
        }
    }
}

impl<T: FileExt> Deref for FileLockGuard<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.file
    }
}

impl<T: FileExt> DerefMut for FileLockGuard<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.file
    }
}

impl<T: FileExt> Drop for FileLockGuard<T> {

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

#[derive(Debug)]
pub struct FileLockError<T> {
    pub err: io::Error,
    pub file: T
}

pub type FileLockResult<T> = Result<FileLockGuard<T>, FileLockError<T>>;

pub struct FileLock<T: FileExt> {
    file: T
}

impl<T: FileExt> FileLock<T> {
    pub fn new(file: T) -> FileLock<T> {
        FileLock {
            file
        }
    }

    pub fn lock_shared(self) -> FileLockResult<T> {
        let res = self.file.lock_shared();
        self.make_result(res)
    }

    pub fn lock_exclusive(self) -> FileLockResult<T> {
        let res = self.file.lock_exclusive();
        self.make_result(res)
    }

    pub fn try_lock_shared(self) -> FileLockResult<T> {
        let res = self.file.try_lock_shared();
        self.make_result(res)
    }

    pub fn try_lock_exclusive(self) -> FileLockResult<T> {
        let res = self.file.try_lock_exclusive();
        self.make_result(res)
    }

    fn make_result(self, res: io::Result<()>) -> FileLockResult<T> {
        let file = self.file;
        match res {
            Ok(()) => Ok(FileLockGuard::new(file)),
            Err(err) => Err(FileLockError{err, file})
        }
    }
}
