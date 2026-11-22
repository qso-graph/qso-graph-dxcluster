// SPDX-License-Identifier: GPL-3.0-or-later
//! The NCDXF/IARU International Beacon Project schedule (Annex G).
//!
//! Source: the Northern California DX Foundation's beacon schedule,
//! <https://www.ncdxf.org/beacon/>, retrieved 2026-11-22. The beacons and their
//! rotation are published by NCDXF and the IARU for anyone to listen to and use.
//! This file is the only place in the repository the beacon calls appear; the
//! sanitization check exempts exactly these quoted calls, in this file only.
//!
//! Like the rest of the library this reads no clock: the caller supplies UTC time.
use crate::model::{Event, Value};

/// One beacon: its call and NCDXF's name for its location.
pub struct Beacon {
    pub call: &'static str,
    pub place: &'static str,
}

/// The 18 beacons in transmit order: the first is on 14.100 MHz at the start of
/// every three-minute cycle, which begins on each UTC minute divisible by three.
pub const BEACONS: [Beacon; 18] = [
    Beacon {
        call: "4U1UN",
        place: "United Nations",
    },
    Beacon {
        call: "VE8AT",
        place: "Canada",
    },
    Beacon {
        call: "W6WX",
        place: "United States",
    },
    Beacon {
        call: "KH6RS",
        place: "Hawaii",
    },
    Beacon {
        call: "ZL6B",
        place: "New Zealand",
    },
    Beacon {
        call: "VK6RBP",
        place: "Australia",
    },
    Beacon {
        call: "JA2IGY",
        place: "Japan",
    },
    Beacon {
        call: "RR9O",
        place: "Russia",
    },
    Beacon {
        call: "VR2B",
        place: "Hong Kong",
    },
    Beacon {
        call: "4S7B",
        place: "Sri Lanka",
    },
    Beacon {
        call: "ZS6DN",
        place: "South Africa",
    },
    Beacon {
        call: "5Z4B",
        place: "Kenya",
    },
    Beacon {
        call: "4X6TU",
        place: "Israel",
    },
    Beacon {
        call: "OH2B",
        place: "Finland",
    },
    Beacon {
        call: "CS3B",
        place: "Madeira",
    },
    Beacon {
        call: "LU4AA",
        place: "Argentina",
    },
    Beacon {
        call: "OA4B",
        place: "Peru",
    },
    Beacon {
        call: "YV5B",
        place: "Venezuela",
    },
];

/// The five beacon frequencies, lowest first, with their bands.
pub const FREQUENCIES: [(u64, &str); 5] = [
    (14_100_000, "20m"),
    (18_110_000, "17m"),
    (21_150_000, "15m"),
    (24_930_000, "12m"),
    (28_200_000, "10m"),
];

/// Each transmission lasts ten seconds; the cycle is 18 of them.
pub const SLOT_SECONDS: u64 = 10;
pub const CYCLE_SECONDS: u64 = SLOT_SECONDS * 18;

/// The beacon transmitting on `band` (an index into `FREQUENCIES`) at `utc`
/// seconds since the Unix epoch: each band runs one slot behind the band below it
/// (QGDXC-BCN-0001).
pub fn on_air(utc: u64, band: usize) -> usize {
    let slot = ((utc % CYCLE_SECONDS) / SLOT_SECONDS) as usize;
    (slot + BEACONS.len() - band % FREQUENCIES.len()) % BEACONS.len()
}

/// Whole seconds until the current transmissions end, 1 to 10.
pub fn seconds_left(utc: u64) -> u64 {
    SLOT_SECONDS - utc % SLOT_SECONDS
}

/// The beacon and band a received call and frequency name: the call must equal a
/// beacon's ignoring ASCII case, and the frequency must be within 1 kHz of a beacon
/// frequency (QGDXC-BCN-0002).
pub fn identify(call: &str, hz: u64) -> Option<(usize, usize)> {
    let beacon = BEACONS
        .iter()
        .position(|b| b.call.eq_ignore_ascii_case(call))?;
    let band = FREQUENCIES
        .iter()
        .position(|(f, _)| f.abs_diff(hz) <= 1_000)?;
    Some((beacon, band))
}

/// The beacon and band an event reports, when it is an RBN beacon spot: a `spot`
/// whose `spot_type` is `NCDXF B` and whose `call` and integer `frequency_hz`
/// name a beacon under [`identify`] (QGDXC-BCN-0002).
pub fn report(event: &Event) -> Option<(usize, usize)> {
    if event.kind != "spot" || event.get("spot_type") != Some("NCDXF B") {
        return None;
    }
    let hz = match event.fields.get("frequency_hz") {
        Some(Value::Integer(hz)) => u64::try_from(*hz).ok()?,
        _ => return None,
    };
    identify(event.get("call")?, hz)
}
