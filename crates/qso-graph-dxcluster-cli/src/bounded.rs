// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded physical input shared by application adapters and verification tools.
use std::io::{BufRead, Read};

pub const RECORD_BYTES: usize = 1_048_576;
pub const TEXT_BYTES: usize = 8 * RECORD_BYTES;

pub fn line<R: BufRead>(reader: &mut R, limit: usize) -> Result<Option<Vec<u8>>, String> {
    let mut bytes = Vec::new();
    let n = reader
        .take(limit as u64 + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if n > limit {
        return Err(format!("physical record exceeds {limit}-byte limit"));
    }
    Ok((n != 0).then_some(bytes))
}
pub fn lines(
    path: &std::path::Path,
) -> Result<impl Iterator<Item = Result<String, String>>, String> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path).map_err(|e| e.to_string())?);
    let mut done = false;
    Ok(std::iter::from_fn(move || {
        if done {
            return None;
        }
        match line(&mut reader, RECORD_BYTES) {
            Ok(Some(bytes)) => Some(String::from_utf8(bytes).map_err(|e| e.to_string())),
            Ok(None) => {
                done = true;
                None
            }
            Err(e) => {
                done = true;
                Some(Err(e))
            }
        }
    }))
}
pub fn read(path: impl AsRef<std::path::Path>) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(TEXT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > TEXT_BYTES {
        return Err(std::io::Error::other("text file exceeds 8 MiB limit"));
    }
    Ok(bytes)
}
pub fn text(path: impl AsRef<std::path::Path>) -> std::io::Result<String> {
    String::from_utf8(read(path)?).map_err(std::io::Error::other)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_records_stop_reading_at_the_byte_cap() {
        // QGDXC-SEC-0001
        let mut input = std::io::BufReader::new(std::io::repeat(b'x').take(10_000_000));
        assert!(line(&mut input, 1024).unwrap_err().contains("1024-byte"));
        assert_eq!(line(&mut &b"a\nb\n"[..], 2).unwrap().unwrap(), b"a\n");
    }
}
