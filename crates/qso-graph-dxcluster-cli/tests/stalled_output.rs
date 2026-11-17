// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(target_os = "linux")]
#![forbid(unsafe_code)]
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait_for(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while !predicate() {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
    true
}
// On a timeout, say which step stalled and what the child reported.
fn expect(step: &str, done: bool, child: &mut Process, stderr: &std::path::Path) {
    if !done {
        let status = child.0.try_wait().ok().flatten();
        let log = fs::read_to_string(stderr).unwrap_or_default();
        panic!("{step} timed out; child status {status:?}; child stderr:\n{log}");
    }
}

#[test]
fn stalled_pipe_cancellation_closes_and_records_before_bounded_exit() {
    // QGDXC-IO-0002 QGDXC-IO-0005 QGDXC-REC-0001 QGDXC-REC-0004
    let listener = TcpListener::bind(("localhost", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let scratch = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let recording = scratch.join(format!("stalled-output-{}.ndjson", std::process::id()));
    let stderr = scratch.join(format!("stalled-output-{}.stderr", std::process::id()));
    let _ = fs::remove_file(&recording);
    let mut child = Process(
        Command::new(env!("CARGO_BIN_EXE_qg-dxc"))
            .args([
                "connect",
                &address.ip().to_string(),
                &address.port().to_string(),
                "--call",
                "Q1ABC",
                "--profile",
                "arcluster",
                "--json",
                "--retain-third-party-data",
                "--record",
            ])
            .arg(&recording)
            .stdout(Stdio::piped()) // Keep the reader open without consuming it.
            .stderr(fs::File::create(&stderr).unwrap())
            .spawn()
            .unwrap(),
    );
    let mut accepted = None;
    let done = wait_for(Duration::from_secs(5), || match listener.accept() {
        Ok((socket, _)) => {
            accepted = Some(socket);
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
        Err(e) => panic!("accept: {e}"),
    });
    expect("accept", done, &mut child, &stderr);
    let mut socket = accepted.unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    socket.write_all(b"call:").unwrap();
    let mut login = [0; 7];
    socket.read_exact(&mut login).unwrap();
    assert_eq!(&login, b"Q1ABC\r\n");
    socket
        .write_all(b"Q1ABC de Q2ABC 1-Nov 1832Z arc6>\n")
        .unwrap();
    socket.set_nonblocking(true).unwrap();
    let line = format!("Q {}\n", "Q".repeat(900));
    let feed = line.repeat(512);
    let mut sent = 0;
    let done = wait_for(Duration::from_secs(5), || {
        match socket.write(&feed.as_bytes()[sent..]) {
            Ok(n) => sent += n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("feed: {e}"),
        }
        sent == feed.len()
    });
    expect("feed", done, &mut child, &stderr);
    // Prove that the child has actually reached a blocked pipe write.
    let done = wait_for(Duration::from_secs(5), || {
        fs::read_dir(format!("/proc/{}/task", child.0.id()))
            .unwrap()
            .filter_map(Result::ok)
            .any(|task| {
                fs::read_to_string(task.path().join("wchan"))
                    .is_ok_and(|state| state.trim() == "pipe_write")
            })
    });
    expect("pipe_write", done, &mut child, &stderr);
    // Once the queue fills, recording stops admitting wire input too.
    let mut previous = 0;
    let done = wait_for(Duration::from_secs(5), || {
        let length = fs::metadata(&recording).unwrap().len();
        let stable = length == previous && length > 0;
        previous = length;
        thread::sleep(Duration::from_millis(100));
        stable && fs::metadata(&recording).unwrap().len() == length
    });
    expect("recording stable", done, &mut child, &stderr);
    let input_count: usize = fs::read_to_string(&recording)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|v| v["dir"] == "in")
        .map(|v| {
            qso_graph_dxcluster::model::unbase64(v["b64"].as_str().unwrap())
                .unwrap()
                .len()
        })
        .sum();
    assert!(input_count < feed.len(), "socket input did not pause");

    let cancelled = Instant::now();
    assert!(
        Command::new("kill")
            .args(["-INT", &child.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let done = wait_for(Duration::from_millis(750), || {
        let text = fs::read_to_string(&recording).unwrap();
        text.lines()
            .last()
            .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .is_some_and(|v| v["conn"] == "close" && v["reason"] == "cancelled")
    });
    expect("close recorded", done, &mut child, &stderr);
    let done = wait_for(Duration::from_millis(250), || {
        match socket.read(&mut [0; 64]) {
            Ok(0) => true,
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => true,
            Ok(_) => false,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
            Err(e) => panic!("closure: {e}"),
        }
    });
    expect("socket closed", done, &mut child, &stderr);
    let mut status = None;
    let done = wait_for(Duration::from_secs(2), || {
        status = child.0.try_wait().unwrap();
        status.is_some()
    });
    expect("exit", done, &mut child, &stderr);
    assert_eq!(status.unwrap().code(), Some(130));
    assert!(cancelled.elapsed() < Duration::from_millis(1750));
    // Validate the complete recording, including the final close boundary.
    let file = fs::File::open(&recording).unwrap();
    let mut records =
        qso_graph_dxcluster_cli::recording::Records::new(std::io::BufReader::new(file));
    while records.next_record().unwrap().is_some() {}
    fs::remove_file(recording).unwrap();
}
