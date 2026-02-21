//! Custom serial protocol for firmware update.
//!
//! Text + binary protocol, newline-delimited.
//!
//! Commands (Host → Device):
//!   PING\n                                    — handshake
//!   INFO\n                                    — query device info
//!   ERASE\n                                   — erase staging region
//!   FLASH <offset:u32> <len:u16> <crc16:u16>\n <binary>  — write chunk
//!   COMMIT <total_size:u32> <crc32:u32>\n     — verify and commit
//!
//! Responses (Device → Host):
//!   PONG\n
//!   INFO <platform> <staging_size> <chunk_size>\n
//!   OK\n
//!   ERR <msg>\n

use crate::bootloader::FlashError;
use vstd::prelude::*;

verus! {

/// Maximum header line length (before binary payload).
pub const MAX_LINE_LEN: usize = 128;

/// Parsed command from the host.
#[derive(Debug)]
pub enum Command {
    Ping,
    Info,
    Erase,
    Flash { offset: u32, len: u16, crc16: u16 },
    Commit { total_size: u32, crc32: u32 },
}

/// Response sent back to the host.
pub enum Response {
    Pong,
    Info {
        platform: &'static str,
        staging_size: u32,
        chunk_size: u32,
    },
    Ok,
    Err(ResponseError),
}

/// Error detail for ERR responses.
pub enum ResponseError {
    Flash(FlashError),
    Protocol(&'static str),
    BadCrc,
}

impl Response {
    /// Serialize the response into a byte buffer, returning the number of bytes written.
    pub fn write_to(&self, buf: &mut [u8]) -> usize {
        match self {
            Self::Pong => copy_str(buf, "PONG\n"),
            Self::Info {
                platform,
                staging_size,
                chunk_size,
            } => {
                // "INFO <platform> <staging_size> <chunk_size>\n"
                let mut pos = copy_str(buf, "INFO ");
                pos += copy_str(&mut buf[pos..], platform);
                pos += copy_str(&mut buf[pos..], " ");
                pos += write_u32(&mut buf[pos..], *staging_size);
                pos += copy_str(&mut buf[pos..], " ");
                pos += write_u32(&mut buf[pos..], *chunk_size);
                pos += copy_str(&mut buf[pos..], "\n");
                pos
            }
            Self::Ok => copy_str(buf, "OK\n"),
            Self::Err(e) => {
                let mut pos = copy_str(buf, "ERR ");
                pos += match e {
                    ResponseError::Flash(FlashError::EraseFailed) => {
                        copy_str(&mut buf[pos..], "erase_failed")
                    }
                    ResponseError::Flash(FlashError::WriteFailed) => {
                        copy_str(&mut buf[pos..], "write_failed")
                    }
                    ResponseError::Flash(FlashError::ReadFailed) => {
                        copy_str(&mut buf[pos..], "read_failed")
                    }
                    ResponseError::Flash(FlashError::CrcMismatch) => {
                        copy_str(&mut buf[pos..], "crc_mismatch")
                    }
                    ResponseError::Flash(FlashError::OutOfBounds) => {
                        copy_str(&mut buf[pos..], "out_of_bounds")
                    }
                    ResponseError::Protocol(msg) => copy_str(&mut buf[pos..], msg),
                    ResponseError::BadCrc => copy_str(&mut buf[pos..], "bad_crc"),
                };
                pos += copy_str(&mut buf[pos..], "\n");
                pos
            }
        }
    }
}

/// Parser state machine — fed one byte at a time.
pub struct ProtocolParser {
    line_buf: [u8; MAX_LINE_LEN],
    line_pos: usize,
    /// After parsing a FLASH header, the parser expects this many binary bytes.
    binary_remaining: u16,
    /// Accumulates binary payload for the current FLASH command.
    binary_buf: [u8; 4096],
    binary_pos: usize,
    /// Stored FLASH header fields while collecting binary data.
    flash_offset: u32,
    flash_len: u16,
    flash_crc16: u16,
}

impl ProtocolParser {
    /// Structural invariant: all indices stay within buffer bounds.
    pub open spec fn inv(&self) -> bool {
        self.line_pos <= MAX_LINE_LEN
        && self.binary_pos <= 4096
        && self.binary_remaining <= 4096
    }
}

impl ProtocolParser {
    pub fn new() -> (result: Self)
        ensures result.inv(),
    {
        Self {
            line_buf: [0u8; MAX_LINE_LEN],
            line_pos: 0,
            binary_remaining: 0,
            binary_buf: [0u8; 4096],
            binary_pos: 0,
            flash_offset: 0,
            flash_len: 0,
            flash_crc16: 0,
        }
    }

    /// Feed a single byte. Returns `Some(Command)` when a complete command is parsed.
    pub fn feed(&mut self, byte: u8) -> (result: Option<Command>)
        requires old(self).inv(),
        ensures self.inv(),
    {
        // If we're collecting binary payload for a FLASH command
        if self.binary_remaining > 0 {
            if self.binary_pos < self.binary_buf.len() {
                self.binary_buf[self.binary_pos] = byte;
            }
            self.binary_pos += 1;
            self.binary_remaining -= 1;

            if self.binary_remaining == 0 {
                return Some(Command::Flash {
                    offset: self.flash_offset,
                    len: self.flash_len,
                    crc16: self.flash_crc16,
                });
            }
            return None;
        }

        // Collecting a text line
        if byte == b'\n' {
            let cmd = self.parse_line();
            self.line_pos = 0;
            return cmd;
        }

        // Ignore \r
        if byte == b'\r' {
            return None;
        }

        if self.line_pos < MAX_LINE_LEN {
            self.line_buf[self.line_pos] = byte;
            self.line_pos += 1;
        }

        None
    }

    /// Access the binary payload buffer (valid after a `Command::Flash` is returned).
    pub fn binary_payload(&self) -> (result: &[u8])
        requires self.inv(),
        ensures result.len() <= 4096,
    {
        &self.binary_buf[..self.binary_pos]
    }

    fn parse_line(&mut self) -> Option<Command> {
        // Copy line to avoid borrowing self.line_buf while calling &mut self methods
        let mut line_copy = [0u8; MAX_LINE_LEN];
        let len = self.line_pos;
        line_copy[..len].copy_from_slice(&self.line_buf[..len]);
        let line = &line_copy[..len];

        if line == b"PING" {
            return Some(Command::Ping);
        }
        if line == b"INFO" {
            return Some(Command::Info);
        }
        if line == b"ERASE" {
            return Some(Command::Erase);
        }

        // FLASH <offset> <len> <crc16>
        if line.starts_with(b"FLASH ") {
            return self.parse_flash_header(line);
        }

        // COMMIT <total_size> <crc32>
        if line.starts_with(b"COMMIT ") {
            return Self::parse_commit(line);
        }

        None
    }

    fn parse_flash_header(&mut self, line: &[u8]) -> Option<Command> {
        // "FLASH <offset> <len> <crc16>"
        let rest = &line[6..]; // skip "FLASH "
        let mut parts = ByteSplitter::new(rest, b' ');

        let offset = parts.next().and_then(parse_u32)?;
        let len = parts.next().and_then(parse_u16)?;
        let crc16 = parts.next().and_then(parse_u16)?;

        // Store header and switch to binary collection mode
        self.flash_offset = offset;
        self.flash_len = len;
        self.flash_crc16 = crc16;
        self.binary_remaining = len;
        self.binary_pos = 0;

        None // Command will be emitted after binary payload is received
    }

    fn parse_commit(line: &[u8]) -> Option<Command> {
        // "COMMIT <total_size> <crc32>"
        let rest = &line[7..]; // skip "COMMIT "
        let mut parts = ByteSplitter::new(rest, b' ');

        let total_size = parts.next().and_then(parse_u32)?;
        let crc32 = parts.next().and_then(parse_u32)?;

        Some(Command::Commit { total_size, crc32 })
    }
}

/// Simple byte-slice splitter (no alloc).
struct ByteSplitter<'a> {
    data: &'a [u8],
    sep: u8,
    pos: usize,
}

impl<'a> ByteSplitter<'a> {
    fn new(data: &'a [u8], sep: u8) -> Self {
        Self { data, sep, pos: 0 }
    }

    fn next(&mut self) -> Option<&'a [u8]> {
        if self.pos >= self.data.len() {
            return None;
        }
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != self.sep {
            self.pos += 1;
        }
        let end = self.pos;
        if self.pos < self.data.len() {
            self.pos += 1; // skip separator
        }
        if start == end {
            None
        } else {
            Some(&self.data[start..end])
        }
    }
}

/// Parse ASCII decimal bytes into u32. Uses checked arithmetic — never overflows.
fn parse_u32(bytes: &[u8]) -> (result: Option<u32>)
    ensures result.is_some() ==> bytes.len() > 0,
{
    let mut result: u32 = 0;
    for &b in bytes {
        if b.is_ascii_digit() {
            result = result.checked_mul(10)?;
            result = result.checked_add(u32::from(b - b'0'))?;
        } else {
            return None;
        }
    }
    if bytes.is_empty() { None } else { Some(result) }
}

/// Parse ASCII decimal bytes into u16.
fn parse_u16(bytes: &[u8]) -> Option<u16> {
    let val = parse_u32(bytes)?;
    u16::try_from(val).ok()
}

/// Copy a str into a byte buffer, returning bytes written. Never exceeds buf.len().
fn copy_str(buf: &mut [u8], s: &str) -> (result: usize)
    ensures result <= buf.len() && result <= s.len(),
{
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len());
    buf[..len].copy_from_slice(&bytes[..len]);
    len
}

/// Write a u32 as ASCII decimal into a buffer, returning bytes written.
/// At most 10 digits (u32::MAX = 4294967295). Never exceeds buf.len().
fn write_u32(buf: &mut [u8], val: u32) -> (result: usize)
    ensures result <= buf.len() && result <= 10,
{
    if val == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
            return 1;
        }
        return 0;
    }

    // Write digits in reverse into a small stack buffer
    let mut tmp = [0u8; 10];
    let mut pos = 0;
    let mut v = val;
    while v > 0 {
        tmp[pos] = b'0' + (v % 10) as u8;
        v /= 10;
        pos += 1;
    }

    let len = pos.min(buf.len());
    for i in 0..len {
        buf[i] = tmp[pos - 1 - i];
    }
    len
}

} // verus!
