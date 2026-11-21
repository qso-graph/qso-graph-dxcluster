# Working on QG DX Cluster

- Public repository: files describe this project only. No lab, host, agent, or internal-process names.
- Follow the [Google Developer Documentation Style Guide](https://developers.google.com/style); American English.
- SPDX `GPL-3.0-or-later` header on every new file, in that format's comment syntax.
- Invented callsigns use the Q prefix (Q1ABC, Q9XYZ-#). Never put a real callsign, IP address, host name
  of a private node, or e-mail address in a vector, fixture, or example. Public node host names may be
  cited as sources only.
- Credentials (node passwords) live in the OS credential store; never in code, config, logs, or examples.
  A secret is never sent over plain Telnet or unverified TLS (QGDXC-SES-0003).
- The library is sans-I/O: it never opens sockets, files or processes. The CLI `qg-dxc` owns the socket.
- Everything a node sends is untrusted: bounded line lengths, control characters never passed to a
  terminal, fuzzed parsers, nothing from the stream ever executed or sent back as a command.
- One fact, one place: every normative statement lives once in the specification, with an ID; other
  files cite the ID.
- Pin each server-specific statement to its evidence: a source file and line, an archived page, or a
  captured session (cited by capture name and line, never reproduced raw).
- Commit messages are Conventional Commits (`feat(tui): …`, `fix: …`, `docs: …`); `cog check` in
  `cargo xtask verify` rejects any other form since the latest release tag. Never edit `CHANGELOG.md`
  by hand: the release pull request writes its section with `scripts/changelog-release.sh`.
