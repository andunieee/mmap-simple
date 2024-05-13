#[derive(Copy, Clone, Debug)]
pub enum MmapError {
    /// # The following are POSIX-specific
    ///
    /// fd was not open for reading or, if using `MapWritable`, was not open for
    /// writing.
    ErrFdNotAvail,
    /// fd was not valid
    ErrInvalidFd,
    /// Either the address given by `MapAddr` or offset given by `MapOffset` was
    /// not a multiple of `MemoryMap::granularity` (unaligned to page size).
    ErrUnaligned,
    /// With `MapFd`, the fd does not support mapping.
    ErrNoMapSupport,
    /// If using `MapAddr`, the address + `min_len` was outside of the process's
    /// address space. If using `MapFd`, the target of the fd didn't have enough
    /// resources to fulfill the request.
    ErrNoMem,
    /// A zero-length map was requested. This is invalid according to
    /// [POSIX](http://pubs.opengroup.org/onlinepubs/9699919799/functions/mmap.html).
    /// Not all platforms obey this, but this wrapper does.
    ErrZeroLength,
    /// Unrecognized error. The inner value is the unrecognized errno.
    ErrUnknown(isize),
    /// # The following are Windows-specific
    ///
    /// Unsupported combination of protection flags
    /// (`MapReadable`/`MapWritable`/`MapExecutable`).
    ErrUnsupProt,
    /// When using `MapFd`, `MapOffset` was given (Windows does not support this
    /// at all)
    ErrUnsupOffset,
    /// When using `MapFd`, there was already a mapping to the file.
    ErrAlreadyExists,
    /// Unrecognized error from `VirtualAlloc`. The inner value is the return
    /// value of GetLastError.
    ErrVirtualAlloc(i32),
    /// Unrecognized error from `CreateFileMapping`. The inner value is the
    /// return value of `GetLastError`.
    ErrCreateFileMappingW(i32),
    /// Unrecognized error from `MapViewOfFile`. The inner value is the return
    /// value of `GetLastError`.
    ErrMapViewOfFile(i32),
}

impl std::fmt::Display for MmapError {
    fn fmt(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
        let str = match *self {
            MmapError::ErrFdNotAvail => "fd not available for reading or writing",
            MmapError::ErrInvalidFd => "Invalid fd",
            MmapError::ErrUnaligned => {
                "Unaligned address, invalid flags, negative length or \
                 unaligned offset"
            }
            MmapError::ErrNoMapSupport => "File doesn't support mapping",
            MmapError::ErrNoMem => "Invalid address, or not enough available memory",
            MmapError::ErrUnsupProt => "Protection mode unsupported",
            MmapError::ErrUnsupOffset => "Offset in virtual memory mode is unsupported",
            MmapError::ErrAlreadyExists => "File mapping for specified file already exists",
            MmapError::ErrZeroLength => "Zero-length mapping not allowed",
            MmapError::ErrUnknown(code) => return write!(out, "Unknown error = {}", code),
            MmapError::ErrVirtualAlloc(code) => {
                return write!(out, "VirtualAlloc failure = {}", code)
            }
            MmapError::ErrCreateFileMappingW(code) => {
                return write!(out, "CreateFileMappingW failure = {}", code)
            }
            MmapError::ErrMapViewOfFile(code) => {
                return write!(out, "MapViewOfFile failure = {}", code)
            }
        };
        write!(out, "{}", str)
    }
}

impl std::error::Error for MmapError {
    fn description(&self) -> &str {
        "memory map error"
    }
}
