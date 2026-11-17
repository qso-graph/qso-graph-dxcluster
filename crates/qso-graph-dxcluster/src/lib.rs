// SPDX-License-Identifier: GPL-3.0-or-later
//! Receive-only DX cluster parsing and session control (QGDXC-SPEC-001 0.1.0).
//! Feed one byte at a time and consume each returned batch before admitting more
//! input. No I/O or clock is used; all times are caller-supplied milliseconds.
#![forbid(unsafe_code)]
pub mod framing;
pub mod model;
pub mod parser;
pub mod privacy;
pub mod session;
pub mod values;
pub use model::{Event, Profile, Value};
pub use parser::{Context, Parser};
pub use session::{Action, Batch, Config, Session, State, Timer};
