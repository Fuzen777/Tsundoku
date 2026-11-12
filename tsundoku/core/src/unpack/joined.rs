//! Pieces of one archive ("Album.7z.001", "Album.7z.002"…) read as the one file they were cut from.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// Files read one after another as if they were one, with seeking across them.
pub(super) struct Joined {
    /// Each piece and where it starts in the whole.
    pieces: Vec<(File, u64)>,
    /// Where each piece's file is at, so reading on from there needs no seek.
    cursors: Vec<u64>,
    length: u64,
    position: u64,
}

impl Joined {
    pub(super) fn open(paths: &[&Path]) -> io::Result<Self> {
        let mut pieces = Vec::with_capacity(paths.len());
        let mut length = 0_u64;
        for path in paths {
            let file = File::open(path)?;
            let size = file.metadata()?.len();
            pieces.push((file, length));
            length += size;
        }
        Ok(Self {
            cursors: vec![0; pieces.len()],
            pieces,
            length,
            position: 0,
        })
    }

    /// The piece holding `position`, and where in it.
    fn piece_at(&self, position: u64) -> Option<(usize, u64)> {
        if position >= self.length {
            return None;
        }
        let index = self
            .pieces
            .partition_point(|(_, start)| *start <= position)
            .saturating_sub(1);
        Some((index, position - self.pieces[index].1))
    }
}

impl Read for Joined {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            let Some((index, offset)) = self.piece_at(self.position) else {
                return Ok(0);
            };
            let (file, _) = &mut self.pieces[index];
            if self.cursors[index] != offset {
                file.seek(SeekFrom::Start(offset))?;
            }
            let read = file.read(buffer)?;
            self.cursors[index] = offset + read as u64;
            if read > 0 || buffer.is_empty() {
                self.position += read as u64;
                return Ok(read);
            }
            // A piece shorter than when it was measured: go on with the next one.
            let next = self
                .pieces
                .get(index + 1)
                .map_or(self.length, |(_, start)| *start);
            if next <= self.position {
                return Ok(0);
            }
            self.position = next;
        }
    }
}

impl Seek for Joined {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let target = match from {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::End(offset) => self.length.checked_add_signed(offset),
            SeekFrom::Current(offset) => self.position.checked_add_signed(offset),
        };
        let target = target
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        self.position = target;
        Ok(target)
    }
}
