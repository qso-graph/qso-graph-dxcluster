// SPDX-License-Identifier: GPL-3.0-or-later
/// Bounded application frame. A push can finish a pending CR and a second frame.
#[derive(Clone, Debug)]
pub struct Frame {
    pub bytes: Vec<u8>,
    pub ending: &'static str,
    pub reason: Option<&'static str>,
}
#[derive(Clone, Debug)]
pub struct Framer {
    buf: Vec<u8>,
    cr: bool,
    discard: bool,
    limit: usize,
}
impl Framer {
    pub fn new(limit: usize) -> Result<Self, &'static str> {
        if !(256..=65536).contains(&limit) {
            return Err("line limit must be 256–65536 bytes");
        }
        Ok(Self {
            buf: Vec::new(),
            cr: false,
            discard: false,
            limit,
        })
    }
    pub fn content(&self) -> &[u8] {
        &self.buf
    }
    pub fn consume_prompt(&mut self) -> Frame {
        Frame {
            bytes: std::mem::take(&mut self.buf),
            ending: "none",
            reason: None,
        }
    }
    fn finish(&mut self, ending: &'static str) -> Option<Frame> {
        if self.discard {
            self.discard = false;
            self.buf.clear();
            return None;
        }
        let bytes = std::mem::take(&mut self.buf);
        let reason = bytes.iter().all(|b| *b == b' ').then_some("blank");
        Some(Frame {
            bytes,
            ending,
            reason,
        })
    }
    pub fn push(&mut self, b: u8) -> Vec<Frame> {
        let mut out = Vec::with_capacity(2);
        if self.cr {
            self.cr = false;
            if b == b'\n' {
                if let Some(f) = self.finish("crlf") {
                    out.push(f);
                }
                return out;
            }
            if let Some(f) = self.finish("cr") {
                out.push(f);
            }
        }
        match b {
            b'\r' => self.cr = true,
            b'\n' => {
                if let Some(f) = self.finish("lf") {
                    out.push(f);
                }
            }
            _ => {
                if !self.discard {
                    if self.buf.len() == self.limit {
                        self.discard = true;
                        out.push(Frame {
                            bytes: std::mem::take(&mut self.buf),
                            ending: "none",
                            reason: Some("line_too_long"),
                        });
                    } else {
                        self.buf.push(b);
                    }
                }
            }
        }
        out
    }
    pub fn eof(&mut self) -> Option<Frame> {
        if self.cr {
            self.cr = false;
            self.finish("cr")
        } else if !self.buf.is_empty() && !self.discard {
            Some(Frame {
                bytes: std::mem::take(&mut self.buf),
                ending: "none",
                reason: Some("partial"),
            })
        } else {
            self.discard = false;
            self.buf.clear();
            None
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Telnet {
    state: u8,
    sb_bytes: usize,
}
impl Telnet {
    /// Returns an application byte, nothing, or a resource-limit failure.
    pub fn push(&mut self, b: u8) -> Result<Option<u8>, &'static str> {
        if self.state >= 3 {
            self.sb_bytes += 1;
            if self.sb_bytes > 4096 {
                return Err("resource_limit");
            }
        }
        match self.state {
            0 => {
                if b == 255 {
                    self.state = 1;
                } else {
                    return Ok(Some(b));
                }
            }
            1 => match b {
                255 => {
                    self.state = 0;
                    return Ok(Some(255));
                }
                250 => {
                    self.state = 3;
                    self.sb_bytes = 2;
                }
                251..=254 => self.state = 2,
                _ => self.state = 0,
            },
            2 => self.state = 0,
            3 => {
                if b == 255 {
                    self.state = 4;
                }
            }
            _ => {
                if b == 240 {
                    self.state = 0;
                    self.sb_bytes = 0;
                } else {
                    self.state = 3;
                }
            }
        }
        Ok(None)
    }
    pub fn eof(&mut self) -> Result<(), &'static str> {
        let sb = self.state >= 3;
        *self = Self::default();
        if sb { Err("resource_limit") } else { Ok(()) }
    }
}
