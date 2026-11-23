<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex I: polite HTTPS (`qso-graph-http`)

`qso-graph-http` is the one place QSO Graph's Rust programs call web services: how often a
provider may be asked, what happens when it says stop, and the HTTPS client that asks. It replaces
the copies in `qg-dxc`'s space-weather fetcher and QSO Graph Desktop's `politeness.rs`, and it is
the crate the Rust MCP servers will build on. It grows in this workspace and moves to
`qso-graph-rs` when its second consumer arrives, under the same name.

**Mechanisms are shared; policies belong to each provider.** The crate enforces floors, waits and
limits; the numbers come from the application, one policy per provider, each with the provider's
stated limit or the recorded judgement and its reason.

## I.1 The ledger

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-HTTP-0001` | `qso-graph-http` MUST be `#![forbid(unsafe_code)]`, MUST depend on no asynchronous runtime, and MUST give a synchronous API whose types are `Send` and `Sync` where they hold no request in progress, so an application on any runtime can use it from a worker thread. Time MUST come from a caller-supplied clock giving monotonic elapsed time and UTC time, never read inside the ledger. | Test, Inspection |
| `QGDXC-HTTP-0002` | A **policy** MUST state, per provider: a refresh floor per request key; whether a wait applies to the one key or to every key of the provider; how `Retry-After` is bounded (a minimum, a maximum, and the wait used when a `429`, or a `503` with `Retry-After`, gives none or one that does not parse); and whether failures without `Retry-After` (other `5xx`, `429` without the header, no connection, timeout) back off: from a first wait, doubling per consecutive failure up to a maximum, or not at all (retry at the floor). | Test |
| `QGDXC-HTTP-0003` | Before each request the ledger MUST answer **go** or **resting until** a time. It MUST answer resting while a wait applies to the key or its provider, or while the key's floor has not passed since its last request. A go MUST count as that key's request whatever the answer, so a provider that fails fast is not asked again sooner. A key the policy does not name MUST be refused, not asked. | Test |
| `QGDXC-HTTP-0004` | After each request the caller MUST report its outcome: answered, busy (`429` or `5xx`, with its `Retry-After` value if any), or unreachable. An answer MUST reset the key's consecutive failures. A busy or unreachable outcome MUST set the wait its policy gives. **A wait in force MUST only ever be lengthened**, never shortened by a later answer, a shorter `Retry-After`, a backoff or a success, since two requests can be outstanding at once. | Test |
| `QGDXC-HTTP-0005` | `Retry-After` MUST be read as delay-seconds (all ASCII digits, saturating on overflow) or an HTTP date in the IMF-fixdate, RFC 850 or asctime form (RFC 9110 §5.6.7, §10.2.3), the date taken relative to the clock's UTC time. Anything else MUST be treated as no value. The parser MUST be covered by a fuzz target. | Test |
| `QGDXC-HTTP-0006` | The two policies in use today MUST both be expressible and MUST be tested as fixtures: **`qg-dxc`'s NOAA policy** (floors of 15 and 5 minutes per product; waits apply to the whole provider; `Retry-After` clamped to 60 s–1 h, 60 s when absent; no backoff) and **QSO Graph Desktop's** (floors per URL; waits per URL; `Retry-After` up to 24 hours, no minimum; failures back off from 1 minute doubling to 1 hour). | Test |

## I.2 The client

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-HTTP-0010` | The client MUST use rustls with the aws-lc-rs provider and a root set the application chooses (the webpki roots for `qg-dxc`), with certificate and host-name verification always on, TLS 1.2 or later, HTTPS only, no proxy from the environment, and no redirects followed. It MUST refuse a request to a host the application did not allow when it built the client. | Test, Inspection |
| `QGDXC-HTTP-0011` | Each request MUST carry a connect timeout and an overall timeout (10 s and 30 s unless the application sets others) and a byte limit, MUST stop reading at the limit plus one byte, and MUST report a body over the limit as an error, not as data. The response MUST give the status, the raw `Retry-After` value if any, and the body. | Test |
| `QGDXC-HTTP-0012` | The `User-Agent` MUST be the one the application supplies. The crate MUST add no identity of its own and MUST refuse a `User-Agent` containing `@`, so an e-mail address cannot be sent by mistake. | Test |
| `QGDXC-HTTP-0013` | `ureq`, `rustls`, `aws-lc-rs` and `aws-lc-sys` MUST be pinned to exact versions in this crate. Only applications MAY depend on `qso-graph-http`; the dependency policy MUST fail if a parsing library (`qso-graph-dxcluster`, `qso-graph-spaceweather` and any later one) depends on it, directly or through others, shown by a planted test. | Test |
| `QGDXC-HTTP-0014` | No test MUST make a network request: the ledger is tested with a fake clock, and the client's settings are tested by inspection of its built configuration and by a local test that reads no socket beyond loopback. | Test, Inspection |
