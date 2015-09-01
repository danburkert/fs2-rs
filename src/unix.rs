extern crate libc;

use std::fs::File;
use std::io::{Error, Result};
use std::os::unix::io::{AsRawFd, FromRawFd};

pub fn duplicate(file: &File) -> Result<File> {
    unsafe {
        let fd = libc::dup(file.as_raw_fd());

        if fd < 0 {
            Err(Error::last_os_error())
        } else {
            Ok(File::from_raw_fd(fd))
        }
    }
}

pub fn lock_shared(file: &File) -> Result<()> {
    flock(file, libc::LOCK_SH)
}

pub fn lock_exclusive(file: &File) -> Result<()> {
    flock(file, libc::LOCK_EX)
}

pub fn lock_shared_nonblock(file: &File) -> Result<()> {
    flock(file, libc::LOCK_SH | libc::LOCK_NB)
}

pub fn lock_exclusive_nonblock(file: &File) -> Result<()> {
    flock(file, libc::LOCK_EX | libc::LOCK_NB)
}

pub fn unlock(file: &File) -> Result<()> {
    flock(file, libc::LOCK_UN)
}

pub fn lock_error() -> Error {
    Error::from_raw_os_error(libc::EWOULDBLOCK)
}

fn flock(file: &File, flag: libc::c_int) -> Result<()> {
    let ret = unsafe { libc::funcs::bsd44::flock(file.as_raw_fd(), flag) };
    if ret < 0 { Err(Error::last_os_error()) } else { Ok(()) }
}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use std::fs;
    use std::os::unix::io::AsRawFd;

    use {FileExt, lock_contended_error};

    /// The duplicate method returns a file with a new file descriptor.
    #[test]
    fn duplicate_new_fd() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().write(true).create(true).open(&path).unwrap();
        let file2 = file1.duplicate().unwrap();
        assert!(file1.as_raw_fd() != file2.as_raw_fd());
    }

    /// Tests that locking a file descriptor will replace any existing locks held on the file
    /// descriptor.
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
        assert_eq!(file2.lock_exclusive_nonblock().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());
        file1.lock_shared().unwrap();
    }

    /// Tests that locks are shared among duplicated file descriptors.
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
        assert_eq!(file3.lock_shared_nonblock().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());

        // Either of the file descriptors should be able to unlock.
        file1.unlock().unwrap();
        file3.lock_shared().unwrap();
    }
}
