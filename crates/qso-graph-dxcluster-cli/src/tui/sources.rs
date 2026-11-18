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
pub struct Workers {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    queues: Vec<SourceQueue>,
    replay: bool,
    next: u64,
    cursor: usize,
}
impl Workers {
    pub fn start(options: Options, origin: Instant, stop: Arc<AtomicBool>) -> Self {
        let replay = matches!(options.sources[0], Source::Replay(_));
        let mut queues = vec![];
        let mut senders = vec![];
        for _ in &options.sources {
            let (tx, rx) = mpsc::sync_channel(CAPACITY);
            senders.push(tx);
            queues.push(SourceQueue {
                rx: Some(rx),
                head: None,
            });
        }
        let mut workers = Self {
            stop,
            threads: vec![],
            queues,
            replay,
            next: 0,
            cursor: 0,
        };
        if replay {
            let stop = workers.stop.clone();
            workers.threads.push(thread::spawn(move || {
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
                workers.threads.push(thread::spawn(move || {
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
        workers
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
        self.stop.store(true, Ordering::Relaxed);
        // Disconnect first: blocked sends wake before joining the sessions.
        for q in &mut self.queues {
            q.rx.take();
            q.head.take();
        }
        for thread in self.threads.drain(..) {
            let _ = thread.join();
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
                let _ = stream.finish();
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
            if let Some(mut stream) = source.stream.take() {
                let _ = stream.finish();
            }
        }
    }
}
impl<R: BufRead> Drop for Schedule<R> {
    fn drop(&mut self) {
        self.cancel();
    }
}
