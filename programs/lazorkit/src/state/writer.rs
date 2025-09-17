use anchor_lang::solana_program::program_memory::sol_memcpy;
use std::cmp;
use std::io::{self, Write};

/// BPF-compatible writer for instruction serialization
///
/// Provides a memory-safe writer implementation that works within Solana's
/// BPF environment for serializing instruction data and account information.

/// BPF-compatible writer for memory-safe data serialization
#[derive(Debug, Default)]
pub struct BpfWriter<T> {
    /// Inner buffer for writing data
    inner: T,
    /// Current position in the buffer
    pos: u64,
}

impl<T> BpfWriter<T> {
    pub fn new(inner: T) -> Self {
        Self { inner, pos: 0 }
    }
}

impl Write for BpfWriter<&mut [u8]> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.pos >= self.inner.len() as u64 {
            return Ok(0);
        }

        let amt = cmp::min(
            self.inner.len().saturating_sub(self.pos as usize),
            buf.len(),
        );
        sol_memcpy(&mut self.inner[(self.pos as usize)..], buf, amt);
        self.pos += amt as u64;
        Ok(amt)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        if self.write(buf)? == buf.len() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "failed to write whole buffer",
            ))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
