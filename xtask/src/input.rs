// SPDX-License-Identifier: GPL-3.0-or-later
//! Fixed budgets for repository inputs and child-process diagnostics.
use qso_graph_dxcluster_cli::bounded::TEXT_BYTES;
pub use qso_graph_dxcluster_cli::bounded::{lines, read, text};
use std::{
    io::{self, Read, Seek},
    process::{Command, Output, Stdio},
};

pub struct Budget(usize);
impl Budget {
    pub fn new() -> Self {
        Self(TEXT_BYTES)
    }
    pub fn charge(&mut self, bytes: usize) -> Result<(), String> {
        self.0 = self
            .0
            .checked_sub(bytes)
            .ok_or("input index exceeds 8 MiB limit")?;
        Ok(())
    }
}
/// Drain both pipes concurrently. Stop a child as soon as either pipe exceeds
/// its cap; waiting for exit before draining could deadlock on a full pipe.
pub fn output(command: &mut Command) -> io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (send, recv) = std::sync::mpsc::channel();
    let other = send.clone();
    let capture = |reader: &mut dyn Read| {
        let mut bytes = Vec::new();
        reader.take(TEXT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
        Ok::<_, io::Error>(bytes)
    };
    let out = std::thread::spawn(move || {
        let _ = send.send((true, capture(&mut { stdout })));
    });
    let err = std::thread::spawn(move || {
        let _ = other.send((false, capture(&mut { stderr })));
    });
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut failure = None;
    for _ in 0..2 {
        match recv.recv().map_err(io::Error::other)? {
            (_, Ok(bytes)) if bytes.len() > TEXT_BYTES => {
                failure = Some(io::Error::other("command output exceeds 8 MiB limit"));
                let _ = child.kill();
            }
            (is_stdout, Ok(bytes)) => {
                if is_stdout {
                    stdout = bytes;
                } else {
                    stderr = bytes;
                }
            }
            (_, Err(e)) => {
                failure = Some(e);
                let _ = child.kill();
            }
        }
    }
    let status = child.wait()?;
    out.join()
        .map_err(|_| io::Error::other("stdout reader failed"))?;
    err.join()
        .map_err(|_| io::Error::other("stderr reader failed"))?;
    if let Some(e) = failure {
        return Err(e);
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}
pub fn tail(path: &str) -> io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    file.seek(io::SeekFrom::Start(len.saturating_sub(65536)))?;
    let mut bytes = Vec::new();
    file.take(65536).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<_> = text.lines().rev().take(20).collect();
    lines.reverse();
    Ok(lines.join("\n"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repository_text_and_diagnostics_have_fixed_caps() {
        // QGDXC-SEC-0001
        let mut budget = Budget::new();
        budget.charge(TEXT_BYTES).unwrap();
        assert!(budget.charge(1).unwrap_err().contains("8 MiB"));
        let path = format!("target/bounded-text-{}", std::process::id());
        std::fs::create_dir_all("target").unwrap();
        let mut file = std::fs::File::create(&path).unwrap();
        file.set_len((TEXT_BYTES + 1) as u64).unwrap();
        assert!(text(&path).unwrap_err().to_string().contains("8 MiB"));
        use std::io::Write;
        file.seek(io::SeekFrom::End(0)).unwrap();
        writeln!(file, "\nlast diagnostic").unwrap();
        let last = tail(&path).unwrap();
        assert!(last.ends_with("last diagnostic"));
        assert!(last.len() <= 65536);
        std::fs::remove_file(path).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn oversized_child_output_is_stopped_without_buffering_it_all() {
        // QGDXC-SEC-0001
        let error = output(Command::new("sh").args(["-c", "yes x"])).unwrap_err();
        assert!(error.to_string().contains("8 MiB"));
    }
}
