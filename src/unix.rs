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

fn flock(file: &File, flag: libc::c_int) -> Result<()> {
    let ret = unsafe { libc::funcs::bsd44::flock(file.as_raw_fd(), flag) };
    if ret < 0 { Err(Error::last_os_error()) } else { Ok(()) }
}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use std::fs;
    use std::os::unix::io::AsRawFd;

    use super::{duplicate};

    /// Tests that the duplicate function returns a file with a new file descriptor.
    #[test]
    fn duplicate_new_fd() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().write(true).create(true).open(&path).unwrap();
        let file2 = duplicate(&file1).unwrap();
        assert!(file1.as_raw_fd() != file2.as_raw_fd());
    }
}
