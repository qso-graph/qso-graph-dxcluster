// SPDX-License-Identifier: GPL-3.0-or-later
use std::{error, fmt};

/// A band condition under QGDXC-SWX-0012.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BandStatus {
    Excellent,
    Good,
    Fair,
    Poor,
    Sporadic,
}

impl BandStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Excellent => "Excellent",
            Self::Good => "Good",
            Self::Fair => "Fair",
            Self::Poor => "Poor",
            Self::Sporadic => "Sporadic",
        }
    }
}

/// The outlook for one band.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BandOutlook {
    pub band: &'static str,
    pub status: BandStatus,
    pub comment: &'static str,
}

/// The input refused by the outlook under QGDXC-SWX-0012.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutlookError {
    InvalidSfi,
    InvalidKp,
}

impl fmt::Display for OutlookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSfi => f.write_str("SFI must be finite and in 0–999"),
            Self::InvalidKp => f.write_str("Kp must be finite and in 0–9"),
        }
    }
}

impl error::Error for OutlookError {}

fn grade(
    band: &'static str,
    index: f64,
    rules: &[(f64, BandStatus, &'static str)],
    otherwise: (BandStatus, &'static str),
) -> BandOutlook {
    let (status, comment) = rules
        .iter()
        .find(|&&(minimum, _, _)| index >= minimum)
        .map_or(otherwise, |&(_, status, comment)| (status, comment));
    BandOutlook {
        band,
        status,
        comment,
    }
}

/// Grade the bands from solar flux and planetary K (QGDXC-SWX-0012).
pub fn band_outlook(sfi: f64, kp: f64) -> Result<[BandOutlook; 10], OutlookError> {
    use BandStatus::{Excellent, Fair, Good, Poor, Sporadic};

    if !(0.0..=999.0).contains(&sfi) {
        return Err(OutlookError::InvalidSfi);
    }
    if !(0.0..=9.0).contains(&kp) {
        return Err(OutlookError::InvalidKp);
    }
    let sfi = sfi.trunc();
    const STORM_KP: f64 = 5.0;
    let mut outlook = [
        grade(
            "160m",
            kp,
            &[
                (STORM_KP, Poor, "Storm absorption"),
                (3.0, Fair, "Some noise"),
            ],
            (Good, "Quiet night conditions"),
        ),
        grade(
            "80m",
            kp,
            &[
                (STORM_KP, Poor, "D-layer absorption and noise"),
                (4.0, Fair, "Elevated absorption"),
            ],
            (Good, "Reliable night band"),
        ),
        grade(
            "40m",
            kp,
            &[(STORM_KP, Fair, "Reduced DX")],
            (Good, "Workhorse band, day and night"),
        ),
        BandOutlook {
            band: "30m",
            status: Good,
            comment: "Transition band, usually open",
        },
        grade(
            "20m",
            sfi,
            &[
                (150.0, Excellent, "Strong F-layer, long openings"),
                (100.0, Good, "Reliable daytime DX"),
            ],
            (Fair, "Shorter openings at low solar flux"),
        ),
        grade(
            "17m",
            sfi,
            &[
                (120.0, Good, "DX openings likely"),
                (90.0, Fair, "Marginal openings"),
            ],
            (Poor, "Insufficient ionization"),
        ),
        grade(
            "15m",
            sfi,
            &[
                (140.0, Excellent, "Wide open DX"),
                (100.0, Good, "Daytime openings"),
                (80.0, Fair, "Sporadic openings"),
            ],
            (Poor, "Band likely closed"),
        ),
        grade(
            "12m",
            sfi,
            &[
                (150.0, Good, "DX openings around midday"),
                (120.0, Fair, "Brief openings possible"),
            ],
            (Poor, "Band likely closed"),
        ),
        grade(
            "10m",
            sfi,
            &[
                (160.0, Excellent, "Worldwide DX"),
                (130.0, Good, "Daytime DX openings"),
                (100.0, Fair, "Short openings or sporadic-E"),
            ],
            (Poor, "Closed except sporadic-E"),
        ),
        BandOutlook {
            band: "6m",
            status: Sporadic,
            comment: "Watch for Es openings",
        },
    ];
    if kp >= STORM_KP {
        // 20m through 10m keep their SFI grade during a storm.
        for entry in &mut outlook[4..9] {
            entry.comment = "Degraded by geomagnetic storm";
        }
    }
    Ok(outlook)
}
