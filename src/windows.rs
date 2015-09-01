extern crate kernel32;
extern crate winapi;

use std::fs::File;
use std::io::{Error, Result};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::ptr;
use std::mem;

pub fn duplicate(file: &File) -> Result<File> {
    unsafe {
        let mut handle = ptr::null_mut();
        let current_process = kernel32::GetCurrentProcess();
        let ret = kernel32::DuplicateHandle(current_process,
                                            file.as_raw_handle(),
                                            current_process,
                                            &mut handle,
                                            0,
                                            true as winapi::BOOL,
                                            winapi::DUPLICATE_SAME_ACCESS);
        if ret == 0 {
            Err(Error::last_os_error())
        } else {
            Ok(File::from_raw_handle(handle))
        }
    }
}

pub fn lock_shared(file: &File) -> Result<()> {
    lock_file(file, 0)
}

pub fn lock_exclusive(file: &File) -> Result<()> {
    lock_file(file, winapi::LOCKFILE_EXCLUSIVE_LOCK)
}

pub fn lock_shared_nonblock(file: &File) -> Result<()> {
    lock_file(file, winapi::LOCKFILE_FAIL_IMMEDIATELY)
}

pub fn lock_exclusive_nonblock(file: &File) -> Result<()> {
    lock_file(file, winapi::LOCKFILE_EXCLUSIVE_LOCK | winapi::LOCKFILE_FAIL_IMMEDIATELY)
}

pub fn unlock(file: &File) -> Result<()> {
    unsafe {
        let ret = kernel32::UnlockFile(file.as_raw_handle(), 0, 0, !0, !0);

        if ret == 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

pub fn lock_error() -> Error {
    Error::from_raw_os_error(winapi::ERROR_LOCK_VIOLATION as i32)
}

fn lock_file(file: &File, flags: winapi::DWORD) -> Result<()> {
    unsafe {
        let mut overlapped = mem::zeroed();
        let ret = kernel32::LockFileEx(file.as_raw_handle(), flags, 0, !0, !0, &mut overlapped);

        if ret == 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {

    extern crate tempdir;

    use std::fs;
    use std::os::windows::io::AsRawHandle;

    use {FileExt, lock_contended_error};

    /// The duplicate method returns a file with a new file handle.
    #[test]
    fn duplicate_new_handle() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().write(true).create(true).open(&path).unwrap();
        let file2 = file1.duplicate().unwrap();
        assert!(file1.as_raw_handle() != file2.as_raw_handle());
    }

    /// A duplicated file handle does not have access to the original handle's locks.
    #[test]
    fn lock_duplicate_handle_independence() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();
        let file2 = file1.duplicate().unwrap();

        // Locking the original file handle will block the duplicate file handle from opening a lock.
        file1.lock_shared().unwrap();
        assert_eq!(file2.lock_exclusive_nonblock().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());

        // Once the original file handle is unlocked, the duplicate handle can proceed with a lock.
        file1.unlock().unwrap();
        file2.lock_exclusive().unwrap();
    }

    /// A file handle may not be exclusively locked multiple times, or exclusively locked and then
    /// shared locked.
    #[test]
    fn lock_non_reentrant() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();

        // Multiple exclusive locks fails.
        file.lock_exclusive().unwrap();
        assert_eq!(file.lock_exclusive_nonblock().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());
        file.unlock().unwrap();

        // Shared then Exclusive locks fails.
        file.lock_shared().unwrap();
        assert_eq!(file.lock_exclusive_nonblock().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());
    }

    /// A file handle can hold an exclusive lock and any number of shared locks, all of which must
    /// be unlocked independently.
    #[test]
    fn lock_layering() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();

        // Open two shared locks on the file, and then try and fail to open an exclusive lock.
        file.lock_exclusive().unwrap();
        file.lock_shared().unwrap();
        file.lock_shared().unwrap();
        assert_eq!(file.lock_exclusive_nonblock().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());

        // Pop one of the shared locks and try again.
        file.unlock().unwrap();
        assert_eq!(file.lock_exclusive_nonblock().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());

        // Pop the second shared lock and try again.
        file.unlock().unwrap();
        assert_eq!(file.lock_exclusive_nonblock().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());

        // Pop the exclusive lock and finally succeed.
        file.unlock().unwrap();
        file.lock_exclusive().unwrap();
    }

    /// A file handle with multiple open locks will have all locks closed on drop.
    #[test]
    fn lock_layering_cleanup() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();
        let file2 = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();

        // Open two shared locks on the file, and then try and fail to open an exclusive lock.
        file1.lock_shared().unwrap();
        assert_eq!(file2.lock_exclusive_nonblock().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());

        drop(file1);
        file2.lock_exclusive().unwrap();
    }

    /// A file handle's locks will not be released until the original handle and all of its
    /// duplicates have been closed. This on really smells like a bug in Windows.
    #[test]
    fn lock_duplicate_cleanup() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().read(true).create(true).open(&path).unwrap();
        let file2 = file1.duplicate().unwrap();

        // Open a lock on the original handle, then close it.
        file1.lock_shared().unwrap();
        drop(file1);

        // Attempting to create a lock on the file with the duplicate handle will fail.
        assert_eq!(file2.lock_exclusive_nonblock().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());
    }
}
