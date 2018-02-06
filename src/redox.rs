extern crate syscall;

use std::fs::File;
use std::io::{Error, ErrorKind, Result};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;

use FsStats;

fn cvt(result: syscall::Result<usize>) -> Result<usize> {
    result.map_err(|err| Error::from_raw_os_error(err.errno))
}

pub fn duplicate(file: &File) -> Result<File> {
    let fd = cvt(syscall::dup(file.as_raw_fd(), &[]))?;
    Ok(unsafe { File::from_raw_fd(fd) })
}

pub fn lock_shared(file: &File) -> Result<()> {
    Err(Error::new(ErrorKind::Other, "flock not supported yet"))
}

pub fn lock_exclusive(file: &File) -> Result<()> {
    Err(Error::new(ErrorKind::Other, "flock not supported yet"))
}

pub fn try_lock_shared(file: &File) -> Result<()> {
    Err(Error::new(ErrorKind::Other, "flock not supported yet"))
}

pub fn try_lock_exclusive(file: &File) -> Result<()> {
    Err(Error::new(ErrorKind::Other, "flock not supported yet"))
}

pub fn unlock(file: &File) -> Result<()> {
    Err(Error::new(ErrorKind::Other, "flock not supported yet"))
}

pub fn lock_error() -> Error {
    Error::from_raw_os_error(syscall::EWOULDBLOCK)
}

pub fn allocated_size(file: &File) -> Result<u64> {
    file.metadata().map(|m| m.blocks() as u64 * 512)
}

pub fn allocate(file: &File, len: u64) -> Result<()> {
    // No file allocation API available, just set the length if necessary.
    if len > try!(file.metadata()).len() as u64 {
        file.set_len(len)
    } else {
        Ok(())
    }
}

pub fn statvfs(path: &Path) -> Result<FsStats> {
    let stat = {
        let mut stat = syscall::StatVfs::default();

        let fd = cvt(syscall::open(path.as_os_str().as_bytes(), syscall::O_CLOEXEC | syscall::O_STAT))?;

        let res = cvt(syscall::fstatvfs(fd, &mut stat));

        let _ = syscall::close(fd);

        res?;

        stat
    };

    Ok(FsStats {
        free_space: stat.f_bsize as u64 * stat.f_bfree as u64,
        available_space: stat.f_bsize as u64 * stat.f_bavail as u64,
        total_space: stat.f_bsize as u64 * stat.f_blocks as u64,
        allocation_granularity: stat.f_bsize as u64,
    })
}
