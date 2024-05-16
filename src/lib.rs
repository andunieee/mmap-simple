//! A simple API for treating a file basically as an infinite vector that can be written to at any
//! point, appended to, read from and shrinken at will and in a very fast way.
//!
//! The file is memory-mapped with a libc call specifying basically an infinite memory size. But it
//! doesn't consume that amount of memory. Should only be used on Linux and from a single caller/process.
//! All write calls immediately call `sync_all` after them, which is not ideal, but maybe we'll improve
//! later.
//!
//! # Example
//!
//! ```rust
//! use std::path::Path;
//! use mmap_simple::Mmap;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut mmap = Mmap::new(Path::new("example.txt"))?;
//!     mmap.append(b"Hello, world!")?;
//!     mmap.overwrite(0, b"Goodbye")?;
//!     mmap.drop_from_tail(6)?;
//!     mmap.append(b", world!")?;
//!     Ok(())
//! }
//! ```

use std::{fs, io, os::unix::prelude::AsRawFd, path};

mod errors;

use crate::errors::*;
use crate::MmapError::*;

/// A struct that represents a memory-mapped file.
pub struct Mmap {
    file: fs::File,
    ptr: *mut u8,
    pub size: u64,
}

impl Mmap {
    /// Creates a new `Mmap` instance by opening the file at the given `path`.
    ///
    /// # Arguments
    /// * `path` - The path to the file to be memory-mapped.
    ///
    /// # Returns
    /// A `Result` containing the `Mmap` instance or a `MmapError` if the operation fails.
    ///
    /// # Errors
    /// This function may return the following errors:
    /// - `ErrFdNotAvail`: The file descriptor is not available for mapping.
    /// - `ErrInvalidFd`: The file descriptor is invalid.
    /// - `ErrUnaligned`: The mapping is not properly aligned.
    /// - `ErrNoMapSupport`: The file system does not support memory mapping.
    /// - `ErrNoMem`: There is not enough memory available to complete the operation.
    /// - `ErrUnknown(code)`: An unknown error occurred with the given OS error code.
    pub fn new(path: &path::Path) -> Result<Self, MmapError> {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .create(true)
            .open(path)
            .unwrap();

        let size = file.metadata().unwrap().len();

        unsafe {
            let r = libc::mmap(
                std::ptr::null::<*const u8>() as *mut libc::c_void,
                1 << 40,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            );

            if r == libc::MAP_FAILED {
                Err(
                    match io::Error::last_os_error().raw_os_error().unwrap_or(-1) {
                        libc::EACCES => ErrFdNotAvail,
                        libc::EBADF => ErrInvalidFd,
                        libc::EINVAL => ErrUnaligned,
                        libc::ENODEV => ErrNoMapSupport,
                        libc::ENOMEM => ErrNoMem,
                        code => ErrUnknown(code as isize),
                    },
                )
            } else {
                let ptr = r as *mut u8;
                Ok(Mmap { ptr, file, size })
            }
        }
    }

    /// Appends the given data to the end of the memory-mapped file.
    ///
    /// # Arguments
    /// * `data` - The data to be appended to the file.
    ///
    /// # Returns
    /// A `Result` containing the unit type (`()`) or an `io::Error` if the operation fails.
    pub fn append(&mut self, data: &[u8]) -> Result<(), io::Error> {
        self.append_with(data.len(), |w| w.copy_from_slice(data))
    }

    /// Appends data to the end of the memory-mapped file using a custom writer function.
    ///
    /// # Arguments
    /// * `len` - The length of the data to be appended.
    /// * `writer` - A closure that writes the data to the file.
    ///
    /// # Returns
    /// A `Result` containing the unit type (`()`) or an `io::Error` if the operation fails.
    pub fn append_with<F>(&mut self, len: usize, writer: F) -> Result<(), io::Error>
    where
        F: FnOnce(&mut [u8]),
    {
        self.file.set_len(self.size + len as u64)?;
        let slice = unsafe {
            std::slice::from_raw_parts_mut(self.ptr.wrapping_offset(self.size as isize), len)
        };
        writer(slice);
        self.size += len as u64;
        self.file.sync_all()?;
        Ok(())
    }

    /// Overwrites the data at the specified offset in the memory-mapped file.
    ///
    /// # Arguments
    /// * `offset` - The offset in the file where the data should be overwritten.
    /// * `data` - The data to be written.
    ///
    /// # Returns
    /// A `Result` containing the unit type (`()`) or an `io::Error` if the operation fails.
    pub fn overwrite(&self, offset: usize, data: &[u8]) -> Result<(), io::Error> {
        self.overwrite_with(offset, data.len(), |w| w.copy_from_slice(data))
    }

    /// Overwrites data in the memory-mapped file using a custom writer function.
    ///
    /// # Arguments
    /// * `offset` - The offset in the file where the data should be overwritten.
    /// * `len` - The length of the data to be written.
    /// * `writer` - A closure that writes the data to the file.
    ///
    /// # Returns
    /// A `Result` containing the unit type (`()`) or an `io::Error` if the operation fails.
    pub fn overwrite_with<F>(&self, offset: usize, len: usize, writer: F) -> Result<(), io::Error>
    where
        F: FnOnce(&mut [u8]),
    {
        if offset + len > self.size as usize {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }

        let slice = unsafe {
            std::slice::from_raw_parts_mut(self.ptr.wrapping_offset(offset as isize), len)
        };
        writer(slice);
        self.file.sync_all()?;
        Ok(())
    }

    /// Removes the specified amount of data from the end of the memory-mapped file by truncating the file.
    ///
    /// # Arguments
    /// * `len` - The amount of data to be removed from the end of the file.
    ///
    /// # Returns
    /// A `Result` containing the unit type (`()`) or an `io::Error` if the operation fails.
    pub fn drop_from_tail(&mut self, len: usize) -> Result<(), io::Error> {
        self.file.set_len(self.size - len as u64)?;
        self.file.sync_all()?;
        self.size -= len as u64;
        Ok(())
    }

    /// Reads data from the memory-mapped file at the specified offset.
    ///
    /// # Arguments
    /// * `offset` - The offset in the file where the data should be read from.
    /// * `buf` - The buffer to store the read data.
    ///
    /// # Returns
    /// A `Result` containing the number of bytes read or an `io::Error` if the operation fails.
    pub fn read(&self, offset: usize, len: usize) -> Result<Vec<u8>, io::Error> {
        let mut buf = vec![0u8; len];
        self.read_with(offset, len, |b| buf.copy_from_slice(b))?;

        Ok(buf)
    }

    /// Reads data from the memory-mapped file at the specified offset using a custom reader function.
    ///
    /// # Arguments
    /// * `offset` - The offset in the file where the data should be read from.
    /// * `len` - The length of the data to be read.
    /// * `reader` - A closure that reads the data from the file.
    ///
    /// # Returns
    /// A `Result` containing the number of bytes read or an `io::Error` if the operation fails.
    pub fn read_with<F>(&self, offset: usize, len: usize, reader: F) -> Result<(), io::Error>
    where
        F: FnOnce(&[u8]),
    {
        if offset + len > self.size as usize {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }

        let slice = unsafe {
            std::slice::from_raw_parts_mut(self.ptr.wrapping_offset(offset as isize), len)
        };
        reader(slice);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use std::io::Read;

    const TEST_FILE: &str = "/tmp/test-mmap";

    fn setup_file() -> Mmap {
        let path = path::Path::new(TEST_FILE);
        fs::remove_file(path).unwrap();
        Mmap::new(path).unwrap()
    }

    fn check_result(match_: &[u8]) {
        let path = path::Path::new(TEST_FILE);
        let mut file = fs::OpenOptions::new().read(true).open(path).unwrap();
        let mut v = vec![0u8; match_.len()];
        let buf = v.as_mut_slice();
        file.read(buf).unwrap();
        assert_eq!(buf, match_);
    }

    #[test]
    #[serial_test::serial]
    fn letters_loop() {
        let mut mmap_file = setup_file();
        let mut c = b'a';
        loop {
            mmap_file.append(&[c]).unwrap();
            println!("appended {}", c);
            if c == b'z' {
                break;
            }
            c += 1;
        }

        drop(mmap_file);
        check_result("abcdefghijklmnopqrstuvwxyz".as_bytes());
    }

    #[test]
    #[serial_test::serial]
    fn multiple_ops() {
        let mut mmap_file = setup_file();

        mmap_file.append("xxxxx".as_bytes()).unwrap();
        let r = mmap_file.overwrite(2, "overflows".as_bytes());
        assert!(r.is_err());
        check_result("xxxxx".as_bytes());

        mmap_file.append("yyyyy".as_bytes()).unwrap();
        mmap_file.overwrite(3, "wwww".as_bytes()).unwrap();
        check_result("xxxwwwwyyy".as_bytes());

        mmap_file.drop_from_tail(4).unwrap();
        check_result("xxxwww".as_bytes());

        let read = mmap_file.read(1, 3).unwrap();
        assert_eq!(read, "xxw".as_bytes());
    }
}
