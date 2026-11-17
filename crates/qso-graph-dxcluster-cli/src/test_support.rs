// SPDX-License-Identifier: GPL-3.0-or-later

pub(crate) fn memory_child(parts: &[&str]) -> bool {
    let test = parts.join(concat!(":", ":"));
    if std::env::var("QGDXC_MEMORY_CHILD").as_deref() == Ok(test.as_str()) {
        return true;
    }
    let output = std::process::Command::new("bash")
        .args([
            "-c",
            "ulimit -c 0; ulimit -v 65536; exec \"$1\" --exact \"$2\" --test-threads=1",
            "input-memory",
        ])
        .arg(std::env::current_exe().unwrap())
        .arg(&test)
        .env("QGDXC_MEMORY_CHILD", &test)
        // One malloc arena: glibc's per-thread arenas reserve 64 MiB each, which
        // fails under this address-space limit and degrades every allocation to mmap.
        .env("MALLOC_ARENA_MAX", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "64 MiB input ceiling: {output:?}");
    false
}
