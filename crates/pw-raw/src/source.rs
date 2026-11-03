//! Random-access byte sources.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// Random-access reads.
pub trait Source {
    /// Fills `buf` from `offset`; errors if the range is not fully available.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()>;
    /// Total length in bytes.
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Size of the head cache. TIFF headers and IFDs are normally near the start of the file,
/// so one read serves most of the metadata walk.
const HEAD: usize = 256 * 1024;

/// A file with a cached head. Reads outside the head go straight to the file.
#[derive(Debug)]
pub struct FileSource {
    file: File,
    len: u64,
    head: Vec<u8>,
}

impl FileSource {
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        let want = usize::try_from(len).map_or(HEAD, |l| l.min(HEAD));
        let mut head = vec![0u8; want];
        file.read_exact(&mut head)?;
        Ok(Self { file, len, head })
    }
}

impl Source for FileSource {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let end = offset
            .checked_add(buf.len() as u64)
            .filter(|&e| e <= self.len)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "read beyond end of file")
            })?;
        if end <= self.head.len() as u64 {
            buf.copy_from_slice(&self.head[offset as usize..end as usize]);
            return Ok(());
        }
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(buf)
    }

    fn len(&self) -> u64 {
        self.len
    }
}

/// An in-memory byte slice.
#[derive(Debug)]
pub struct SliceSource<'a>(pub &'a [u8]);

impl Source for SliceSource<'_> {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let start = usize::try_from(offset).map_err(|_| eof())?;
        let end = start.checked_add(buf.len()).ok_or_else(eof)?;
        let src = self.0.get(start..end).ok_or_else(eof)?;
        buf.copy_from_slice(src);
        Ok(())
    }

    fn len(&self) -> u64 {
        self.0.len() as u64
    }
}

fn eof() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "read beyond end of data")
}
