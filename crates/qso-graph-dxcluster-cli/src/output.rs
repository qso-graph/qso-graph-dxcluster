// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    collections::VecDeque,
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, TrySendError},
    },
    time::{Duration, Instant},
};

// QGDXC-IO-0005: 64 rendered lines in the channel, one in the writer,
// and 64 pending lines. Input pauses as soon as a line cannot be sent.
// The pending reserve also admits timer/closure events during backpressure;
// the session's finite retry budget bounds these independent of input.
const CAPACITY: usize = 64;
pub(super) struct Output {
    tx: Option<mpsc::SyncSender<String>>,
    pending: VecDeque<String>,
    done: mpsc::Receiver<Result<(), String>>,
    queued: Arc<AtomicUsize>,
}

impl Output {
    pub(super) fn new(mut writer: impl Write + Send + 'static) -> Self {
        let (tx, rx) = mpsc::sync_channel::<String>(CAPACITY);
        let (done_tx, done) = mpsc::sync_channel(1);
        let queued = Arc::new(AtomicUsize::new(0));
        let worker_queued = queued.clone();
        std::thread::spawn(move || {
            let result = (|| {
                for line in rx {
                    worker_queued.fetch_sub(1, Ordering::Relaxed);
                    writeln!(writer, "{line}").map_err(|e| e.to_string())?;
                }
                writer.flush().map_err(|e| e.to_string())
            })();
            let _ = done_tx.send(result);
        });
        Self {
            tx: Some(tx),
            pending: VecDeque::new(),
            done,
            queued,
        }
    }
    pub(super) fn enqueue(&mut self, line: String) -> Result<(), String> {
        self.flush()?;
        if self.pending.len() == CAPACITY {
            return Err("render queue resource_limit".into());
        }
        self.pending.push_back(line);
        self.flush()
    }
    pub(super) fn flush(&mut self) -> Result<(), String> {
        while let Some(line) = self.pending.pop_front() {
            self.queued.fetch_add(1, Ordering::Relaxed);
            match self.tx.as_ref().expect("active writer").try_send(line) {
                Ok(()) => {}
                Err(TrySendError::Full(line)) => {
                    self.queued.fetch_sub(1, Ordering::Relaxed);
                    self.pending.push_front(line);
                    break;
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.queued.fetch_sub(1, Ordering::Relaxed);
                    return Err(self
                        .done
                        .try_recv()
                        .ok()
                        .and_then(Result::err)
                        .unwrap_or_else(|| "output writer ended".into()));
                }
            }
        }
        Ok(())
    }
    pub(super) fn backpressured(&self) -> bool {
        !self.pending.is_empty() || self.queued.load(Ordering::Relaxed) >= CAPACITY
    }
    pub(super) fn finish(mut self, cancelled: impl Fn() -> bool) -> Result<(), String> {
        let mut deadline = None;
        loop {
            // Cancellation can also arrive after transport closure, while a
            // normal drain is already waiting for the pipe to become writable.
            if cancelled() && deadline.is_none() {
                deadline = Some(Instant::now() + Duration::from_secs(1));
            }
            if self.tx.is_some() {
                self.flush()?;
                if self.pending.is_empty() {
                    self.tx.take();
                }
            }
            match self.done.try_recv() {
                Ok(result) => return result,
                Err(mpsc::TryRecvError::Disconnected) => return Err("output writer ended".into()),
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return Err("output drain timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    struct Paused {
        resume: mpsc::Receiver<()>,
        entered: mpsc::SyncSender<()>,
        bytes: Arc<Mutex<Vec<u8>>>,
        first: bool,
    }
    impl Write for Paused {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.first {
                self.first = false;
                self.entered.send(()).unwrap();
                self.resume.recv().unwrap();
            }
            self.bytes.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn bounded_queue_resumes_without_losing_admitted_lines() {
        // QGDXC-IO-0005: exercise the channel and the timer/closure reserve.
        let (resume, rx) = mpsc::channel();
        let (entered, ready) = mpsc::sync_channel(1);
        let bytes = Arc::new(Mutex::new(vec![]));
        let mut output = Output::new(Paused {
            resume: rx,
            entered,
            bytes: bytes.clone(),
            first: true,
        });
        output.enqueue("0".into()).unwrap();
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        for i in 1..=CAPACITY {
            output.enqueue(i.to_string()).unwrap();
        }
        assert!(output.backpressured());
        output.enqueue((CAPACITY + 1).to_string()).unwrap();
        assert!(output.backpressured());
        for i in CAPACITY + 2..=2 * CAPACITY {
            output.enqueue(i.to_string()).unwrap();
        }
        assert_eq!(output.pending.len(), CAPACITY);
        assert!(output.enqueue("excess".into()).is_err());
        resume.send(()).unwrap();
        output.finish(|| false).unwrap();
        let expected: String = (0..=2 * CAPACITY).map(|i| format!("{i}\n")).collect();
        assert_eq!(*bytes.lock().unwrap(), expected.as_bytes());
    }
    #[test]
    fn cancellation_during_a_stalled_drain_has_a_deadline() {
        // QGDXC-IO-0002: Ctrl-C also works after a terminal session outcome.
        let (resume, rx) = mpsc::channel();
        let (entered, ready) = mpsc::sync_channel(1);
        let mut output = Output::new(Paused {
            resume: rx,
            entered,
            bytes: Arc::new(Mutex::new(vec![])),
            first: true,
        });
        output.enqueue("Q".into()).unwrap();
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        let started = Instant::now();
        let result = output.finish(|| started.elapsed() >= Duration::from_millis(100));
        assert!(result.unwrap_err().contains("timed out"));
        assert!(started.elapsed() >= Duration::from_millis(1100));
        assert!(started.elapsed() < Duration::from_millis(1600));
        resume.send(()).unwrap();
    }
}
