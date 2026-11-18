// SPDX-License-Identifier: GPL-3.0-or-later
//! Application-owned source threads and the deterministic offline scheduler.
use super::{
    config::{Options, Source},
    model::Model,
};
use crate::{Delivery, recording::ReplayStream, run_live};
use qso_graph_dxcluster::Event;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const CAPACITY: usize = 1024;
pub enum Message {
    Event(Event),
    Complete,
    Error(String),
}
pub struct Envelope {
    pub order: u64,
    pub message: Message,
}
struct SourceQueue {
    rx: Option<Receiver<Envelope>>,
    head: Option<Envelope>,
}
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
struct Worker {
    thread: JoinHandle<()>,
    done: Receiver<()>,
}
impl Worker {
    fn spawn(work: impl FnOnce() + Send + 'static) -> Self {
        let (tx, done) = mpsc::channel();
        let thread = thread::spawn(move || {
            // Disconnection also signals completion if the worker unwinds.
            let _completion = tx;
            work();
        });
        Self { thread, done }
    }
}
pub struct Workers {
    stop: Arc<AtomicBool>,
    threads: Vec<Worker>,
    queues: Vec<SourceQueue>,
    replay: bool,
    next: u64,
    cursor: usize,
}
impl Workers {
    pub fn new(stop: Arc<AtomicBool>) -> Self {
        Self {
            stop,
            threads: vec![],
            queues: vec![],
            replay: false,
            next: 0,
            cursor: 0,
        }
    }
    pub fn start(&mut self, options: Options, origin: Instant) {
        let replay = matches!(options.sources[0], Source::Replay(_));
        self.replay = replay;
        let mut senders = vec![];
        for _ in &options.sources {
            let (tx, rx) = mpsc::sync_channel(CAPACITY);
            senders.push(tx);
            self.queues.push(SourceQueue {
                rx: Some(rx),
                head: None,
            });
        }
        let workers = self;
        if replay {
            let stop = workers.stop.clone();
            workers.threads.push(Worker::spawn(move || {
                let streams = options
                    .sources
                    .into_iter()
                    .map(|s| match s {
                        Source::Replay(path) => File::open(path)
                            .map_err(|e| e.to_string())
                            .and_then(|f| ReplayStream::new(BufReader::new(f), false)),
                        _ => unreachable!(),
                    })
                    .collect();
                let mut schedule = Schedule::new(streams);
                let mut sequence = 0;
                while !stop.load(Ordering::Relaxed) {
                    let now = origin.elapsed().as_secs_f64();
                    if let Some((source, message)) = schedule.poll(now) {
                        if senders[source]
                            .send(Envelope {
                                order: sequence,
                                message,
                            })
                            .is_err()
                        {
                            break;
                        }
                        sequence += 1;
                    } else if schedule.done() {
                        break;
                    } else {
                        thread::sleep(Duration::from_millis(5));
                    }
                }
                // A cancelled adapter still finalizes its current session once.
                schedule.cancel();
            }));
        } else {
            for (source, tx) in options.sources.into_iter().zip(senders) {
                let Source::Node(config) = source else {
                    unreachable!()
                };
                let stop = workers.stop.clone();
                workers.threads.push(Worker::spawn(move || {
                    // Delivery uses the same bounded source queue directly. No
                    // forwarding queue can hide an unbounded event backlog.
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run_live(config, None, false, stop, Delivery::Tui(tx.clone()))
                    }))
                    .unwrap_or_else(|_| Err("source panic".into()));
                    if let Err(error) = result {
                        let _ = tx.send(Envelope {
                            order: 0,
                            message: Message::Error(error),
                        });
                    }
                }));
            }
        }
    }
    pub fn drain(&mut self, model: &mut Model, origin: Instant) {
        // Bound work per UI turn so input and redraw remain responsive.
        let start = Instant::now();
        for _ in 0..8192 {
            if start.elapsed() >= Duration::from_millis(10) {
                break;
            }
            for queue in &mut self.queues {
                if queue.head.is_none() {
                    queue.head = queue.rx.as_ref().and_then(|rx| rx.try_recv().ok());
                }
            }
            let source = (0..self.queues.len())
                .map(|n| (n + self.cursor) % self.queues.len())
                .find(|i| {
                    self.queues[*i]
                        .head
                        .as_ref()
                        .is_some_and(|h| !self.replay || h.order == self.next)
                });
            let Some(source) = source else {
                break;
            };
            self.cursor = (source + 1) % self.queues.len();
            let message = self.queues[source].head.take().unwrap().message;
            if self.replay {
                self.next += 1;
            }
            match message {
                Message::Event(event) => model.receive(source, event, origin.elapsed()),
                Message::Error(error) => {
                    model.sources[source].diagnostic = format!("ERROR {error}")
                }
                Message::Complete => model.sources[source].complete = true,
            }
        }
    }
}
impl Drop for Workers {
    fn drop(&mut self) {
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        self.stop.store(true, Ordering::Relaxed);
        // Disconnect first: blocked sends wake before joining the sessions.
        for q in &mut self.queues {
            q.rx.take();
            q.head.take();
        }
        // One deadline covers all sources, including non-cancellable DNS/dials.
        for worker in self.threads.drain(..) {
            match worker
                .done
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = worker.thread.join();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Dropping the handle detaches the straggler; process exit
                    // closes its resources under QGDXC-TUI-0052.
                }
            }
        }
    }
}
type Record = (usize, serde_json::Value, Vec<Event>);
struct ReplaySource<R: BufRead> {
    stream: Option<ReplayStream<R>>,
    pending: Option<Record>,
    time: f64,
    output: std::collections::VecDeque<Message>,
}
pub struct Schedule<R: BufRead> {
    sources: Vec<ReplaySource<R>>,
}
impl<R: BufRead> Schedule<R> {
    pub fn new(streams: Vec<Result<ReplayStream<R>, String>>) -> Self {
        let sources = streams
            .into_iter()
            .map(|stream| match stream {
                Ok(stream) => ReplaySource {
                    stream: Some(stream),
                    pending: None,
                    time: 0.0,
                    output: Default::default(),
                },
                Err(e) => ReplaySource {
                    stream: None,
                    pending: None,
                    time: 0.0,
                    output: [Message::Error(e)].into(),
                },
            })
            .collect();
        Self { sources }
    }
    fn advance(source: &mut ReplaySource<R>) {
        if source.pending.is_some() || !source.output.is_empty() {
            return;
        }
        let Some(stream) = &mut source.stream else {
            return;
        };
        match stream.next_record() {
            Ok(Some(record)) => {
                if let Some(t) = record.1["t"].as_f64() {
                    source.time = t;
                }
                source.pending = Some(record);
            }
            Ok(None) => {
                match stream.finish() {
                    Ok(events) => {
                        source.output.extend(events.into_iter().map(Message::Event));
                        source.output.push_back(Message::Complete);
                    }
                    Err(e) => source.output.push_back(Message::Error(e)),
                }
                source.stream = None;
            }
            Err(e) => {
                if let Ok(events) = stream.abort() {
                    source.output.extend(events.into_iter().map(Message::Event));
                }
                source.output.push_back(Message::Error(e));
                source.stream = None;
            }
        }
    }
    pub fn poll(&mut self, now: f64) -> Option<(usize, Message)> {
        for source in &mut self.sources {
            Self::advance(source);
        }
        loop {
            let (index, _) = self
                .sources
                .iter()
                .enumerate()
                .filter(|(_, s)| s.pending.is_some() || !s.output.is_empty())
                .min_by(|(ia, a), (ib, b)| a.time.total_cmp(&b.time).then(ia.cmp(ib)))?;
            let source = &mut self.sources[index];
            if source.time > now {
                return None;
            }
            if let Some(message) = source.output.pop_front() {
                return Some((index, message));
            }
            // Observer notes are recording metadata (the first one names the
            // recorded operator), not session status, so they are not shown.
            let (_, _, events) = source.pending.take().unwrap();
            source.output.extend(events.into_iter().map(Message::Event));
            if source.output.is_empty() {
                Self::advance(source);
            }
        }
    }
    pub fn done(&self) -> bool {
        self.sources
            .iter()
            .all(|s| s.stream.is_none() && s.pending.is_none() && s.output.is_empty())
    }
    pub fn cancel(&mut self) {
        for source in &mut self.sources {
            if let Some(mut stream) = source.stream.take()
                && let Ok(events) = stream.abort()
            {
                source.output.extend(events.into_iter().map(Message::Event));
            }
        }
    }
}
impl<R: BufRead> Drop for Schedule<R> {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{Interface, Screen, ScreenIo, input};
    use crossterm::event::{Event as InputEvent, KeyCode, KeyEvent, KeyModifiers};
    use std::io;

    struct FakeScreen {
        restored: Arc<AtomicBool>,
        stop: Arc<AtomicBool>,
    }
    impl ScreenIo for FakeScreen {
        fn raw(&mut self) -> io::Result<bool> {
            Ok(false)
        }
        fn set_raw(&mut self, enabled: bool) -> io::Result<()> {
            if !enabled {
                self.restored.store(true, Ordering::SeqCst);
            }
            Ok(())
        }
        fn enter_screen(&mut self) -> io::Result<()> {
            Ok(())
        }
        fn leave_screen(&mut self) -> io::Result<()> {
            assert!(
                !self.stop.load(Ordering::SeqCst),
                "sources stopped before terminal restoration"
            );
            Ok(())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn workers(stop: Arc<AtomicBool>) -> Workers {
        Workers {
            stop,
            threads: vec![],
            queues: vec![],
            replay: false,
            next: 0,
            cursor: 0,
        }
    }
    #[test]
    fn schedule_cancel_aborts_pending_cr_partial_line_and_telnet_once() {
        // QGDXC-TUI-0052: exercise the source cleanup path itself.
        use crate::recording::{Recorder, ReplayStream};
        use std::io::Cursor;
        for suffix in [&b"\r"[..], &b""[..], &b"\xff\xfa\x01"[..]] {
            let mut recording = Recorder::new(vec![], true).unwrap();
            recording
                .note(
                    0.0,
                    "qg-dxc configuration: {\"call\":\"Q1ABC\",\"profile\":\"arcluster\"}",
                )
                .unwrap();
            recording.open(0.0).unwrap();
            recording
                .wire(
                    0.0,
                    "in",
                    &[b"DX de Q9XYZ: 14074 Q2ABC CW 1832Z".as_slice(), suffix].concat(),
                )
                .unwrap();
            recording.close(3600.0, "cancelled").unwrap();
            let mut schedule = Schedule::new(vec![Ok(ReplayStream::new(
                Cursor::new(recording.into_inner()),
                false,
            )
            .unwrap())]);
            // Admit the fragment while the future close is still unread.
            let stream = schedule.sources[0].stream.as_mut().unwrap();
            loop {
                let (_, record, _) = stream.next_record().unwrap().unwrap();
                if record["dir"] == "in" {
                    break;
                }
            }
            schedule.cancel();
            schedule.cancel();
            let (mut content, mut ended) = (0, 0);
            while let Some((_, message)) = schedule.poll(0.0) {
                if let Message::Event(event) = message {
                    if event.kind == "spot" || event.kind == "unrecognized_line" {
                        content += 1;
                    }
                    if event.kind == "connection_state" && event.get("reason") == Some("cancelled")
                    {
                        ended += 1;
                    }
                }
            }
            assert_eq!((content, ended), (1, 1), "{suffix:?}");
            assert!(schedule.done());
        }
    }
    #[test]
    fn noncancellable_sources_share_shutdown_deadline_after_terminal_restore() {
        // QGDXC-TUI-0052: two pending, non-cooperative resolver/dialer calls.
        let stop = Arc::new(AtomicBool::new(false));
        let restored = Arc::new(AtomicBool::new(false));
        let mut workers = workers(stop.clone());
        let (started, started_rx) = mpsc::channel();
        let mut releases = vec![];
        for _ in 0..2 {
            let (release, pending) = mpsc::channel();
            releases.push(release);
            let started = started.clone();
            workers.threads.push(Worker::spawn(move || {
                started.send(()).unwrap();
                // Deliberately ignores the stop flag, as blocking DNS can.
                let _ = pending.recv_timeout(Duration::from_secs(30));
            }));
        }
        for _ in 0..2 {
            started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        }
        let screen = Screen::enter(FakeScreen {
            restored: restored.clone(),
            stop,
        })
        .unwrap();
        let interface = Interface {
            _screen: screen,
            workers,
        };
        let mut model = Model::new(vec!["one".into()]);
        model.key(
            input(InputEvent::Key(KeyEvent::new(
                KeyCode::Char('q'),
                KeyModifiers::NONE,
            )))
            .unwrap(),
        );
        assert!(model.quit);
        let start = Instant::now();
        drop(interface);
        let elapsed = start.elapsed();
        for release in releases {
            let _ = release.send(());
        }
        assert!(restored.load(Ordering::SeqCst));
        assert!(elapsed >= SHUTDOWN_TIMEOUT);
        assert!(
            elapsed < SHUTDOWN_TIMEOUT + Duration::from_millis(400),
            "{elapsed:?}"
        );
    }
    #[test]
    fn full_queue_shutdown_restores_terminal_and_completes_workers_on_all_exits() {
        // QGDXC-TUI-0052 QGDXC-TUI-0053
        for outcome in ["quit", "ctrl-c", "error", "unwind"] {
            let stop = Arc::new(AtomicBool::new(false));
            let restored = Arc::new(AtomicBool::new(false));
            let mut workers = workers(stop.clone());
            let (tx, rx) = mpsc::sync_channel(CAPACITY);
            for _ in 0..CAPACITY {
                tx.send(Envelope {
                    order: 0,
                    message: Message::Complete,
                })
                .unwrap();
            }
            workers.queues.push(SourceQueue {
                rx: Some(rx),
                head: None,
            });
            let (started, started_rx) = mpsc::channel();
            let (finished, finished_rx) = mpsc::channel();
            let terminal_restored = restored.clone();
            workers.threads.push(Worker::spawn(move || {
                started.send(()).unwrap();
                let result = tx.send(Envelope {
                    order: 0,
                    message: Message::Complete,
                });
                assert!(
                    result.is_err(),
                    "full queue was drained instead of disconnected"
                );
                assert!(terminal_restored.load(Ordering::SeqCst));
                finished.send(()).unwrap();
            }));
            started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            assert!(finished_rx.recv_timeout(Duration::from_millis(20)).is_err());
            let screen = Screen::enter(FakeScreen {
                restored: restored.clone(),
                stop,
            })
            .unwrap();
            let start = Instant::now();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                move || -> io::Result<()> {
                    let _interface = Interface {
                        _screen: screen,
                        workers,
                    };
                    let mut model = Model::new(vec!["one".into()]);
                    match outcome {
                        "quit" | "ctrl-c" => {
                            let (c, modifiers) = if outcome == "quit" {
                                ('q', KeyModifiers::NONE)
                            } else {
                                ('c', KeyModifiers::CONTROL)
                            };
                            model.key(
                                input(InputEvent::Key(KeyEvent::new(KeyCode::Char(c), modifiers)))
                                    .unwrap(),
                            );
                            assert!(model.quit);
                            Ok(())
                        }
                        "error" => Err(io::Error::other("synthetic interface error")),
                        _ => panic!("synthetic interface unwind"),
                    }
                },
            ));
            finished_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            assert!(
                start.elapsed() < Duration::from_secs(1),
                "{outcome}: worker did not complete promptly"
            );
            assert!(restored.load(Ordering::SeqCst));
            assert_eq!(result.is_err(), outcome == "unwind");
        }
    }
}
