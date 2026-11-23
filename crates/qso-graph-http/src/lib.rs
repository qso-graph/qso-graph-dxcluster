// SPDX-License-Identifier: GPL-3.0-or-later
//! Synchronous HTTPS and provider request scheduling (specification Annex I).

#![forbid(unsafe_code)]

mod client;
mod ledger;
mod retry_after;

pub use client::{Client, Error, Response, ResponseHead, Roots, Timeouts};
pub use ledger::{
    Backoff, Clock, Gate, Ledger, Outcome, Policy, RetryAfter, RetryAfterStatuses, UnknownKey,
    WaitScope,
};
pub use retry_after::parse_retry_after;
