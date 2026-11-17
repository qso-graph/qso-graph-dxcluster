// SPDX-License-Identifier: GPL-3.0-or-later
//! Application-owned recording and offline replay adapters.
#![forbid(unsafe_code)]
pub mod bounded;
pub mod recording;

#[cfg(all(test, target_os = "linux"))]
mod test_support;
