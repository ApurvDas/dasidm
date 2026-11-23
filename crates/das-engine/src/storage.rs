//! Positioned writes straight into the final-offset part file.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

pub fn open_part(path: &Path, size: Option<u64>, create: bool) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let f = OpenOptions::new().read(true).write(true).create(create).truncate(false).open(path)?;
    if create {
        set_sparse(&f);
        if let Some(size) = size {
            if f.metadata()?.len() != size {
                f.set_len(size)?;
            }
        }
    }
    Ok(f)
}

/// Write the whole buffer at `offset`. Safe to call concurrently on one handle.
pub fn write_all_at(f: &File, mut offset: u64, mut buf: &[u8]) -> io::Result<()> {
    while !buf.is_empty() {
        let n = write_at(f, offset, buf)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "disk refused the write"));
        }
        offset += n as u64;
        buf = &buf[n..];
    }
    Ok(())
}

#[cfg(windows)]
fn write_at(f: &File, offset: u64, buf: &[u8]) -> io::Result<usize> {
    use std::os::windows::fs::FileExt;
    f.seek_write(buf, offset)
}

#[cfg(unix)]
fn write_at(f: &File, offset: u64, buf: &[u8]) -> io::Result<usize> {
    use std::os::unix::fs::FileExt;
    f.write_at(buf, offset)
}

/// Mark the file sparse so NTFS does not zero-fill the gap before an out-of-order write.
#[cfg(windows)]
fn set_sparse(f: &File) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::FSCTL_SET_SPARSE;
    let mut returned = 0u32;
    // Best effort: FAT32/exFAT do not support sparse files and simply fail here.
    unsafe {
        DeviceIoControl(
            f.as_raw_handle() as _,
            FSCTL_SET_SPARSE,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        );
    }
}

#[cfg(not(windows))]
fn set_sparse(_f: &File) {}
