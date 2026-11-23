// SPDX-License-Identifier: GPL-3.0-or-later
//! Maidenhead locators and geographic positions (Annex K).
#![forbid(unsafe_code)]

use core::{fmt, str::FromStr};

const SIZES: [(f64, f64); 4] = [
    (20.0, 10.0),
    (2.0, 1.0),
    (2.0 / 24.0, 1.0 / 24.0),
    (2.0 / 240.0, 1.0 / 240.0),
];
const ALPHABETS: [(u8, u8); 4] = [(b'A', 18), (b'0', 10), (b'a', 24), (b'0', 10)];

/// A refused locator or position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Locator,
    Position,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Locator => "invalid Maidenhead locator",
            Self::Position => "position is outside the geographic bounds",
        })
    }
}
impl core::error::Error for Error {}

/// A geographic position in degrees (QGDXC-GRID-0005).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    latitude: f64,
    longitude: f64,
}

impl Position {
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, Error> {
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            return Err(Error::Position);
        }
        Ok(Self {
            latitude,
            longitude,
        })
    }

    pub fn latitude(self) -> f64 {
        self.latitude
    }

    pub fn longitude(self) -> f64 {
        self.longitude
    }
}

/// The edges of a locator's cell, in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

/// The number of character pairs in a locator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Precision {
    Field = 1,
    Square,
    Subsquare,
    Extended,
}

/// A validated Maidenhead locator (QGDXC-GRID-0002).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Locator {
    cells: [[u8; 2]; 4],
    precision: Precision,
}

impl Locator {
    pub fn parse(locator: &str) -> Result<Self, Error> {
        let precision = match locator.len() {
            2 => Precision::Field,
            4 => Precision::Square,
            6 => Precision::Subsquare,
            8 => Precision::Extended,
            _ => return Err(Error::Locator),
        };
        let mut cells = [[0; 2]; 4];
        for (i, pair) in locator.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let (base, size) = ALPHABETS[i];
            for (axis, &byte) in pair.iter().enumerate() {
                let index = byte
                    .to_ascii_uppercase()
                    .wrapping_sub(base.to_ascii_uppercase());
                if index >= size {
                    return Err(Error::Locator);
                }
                cells[i][axis] = index;
            }
        }
        Ok(Self { cells, precision })
    }

    pub fn precision(self) -> Precision {
        self.precision
    }

    pub fn bounds(self) -> Bounds {
        let mut west = -180.0;
        let mut south = -90.0;
        let pairs = self.precision as usize;
        for (i, &(width, height)) in SIZES[..pairs].iter().enumerate() {
            west += f64::from(self.cells[i][0]) * width;
            south += f64::from(self.cells[i][1]) * height;
        }
        let (width, height) = SIZES[pairs - 1];
        Bounds {
            south,
            west,
            north: south + height,
            east: west + width,
        }
    }

    pub fn centre(self) -> Position {
        let Bounds {
            south,
            west,
            north,
            east,
        } = self.bounds();
        Position {
            latitude: (south + north) / 2.0,
            longitude: (west + east) / 2.0,
        }
    }

    pub fn holding(position: Position, precision: Precision) -> Self {
        let mut longitude = (position.longitude + 180.0).min(360.0 - 1e-9);
        let mut latitude = (position.latitude + 90.0).min(180.0 - 1e-9);
        let mut cells = [[0; 2]; 4];
        for (i, &(width, height)) in SIZES[..precision as usize].iter().enumerate() {
            // Keep values rounded onto an upper edge inside the parent cell.
            let x = ((longitude / width).floor() as u8).min(ALPHABETS[i].1 - 1);
            let y = ((latitude / height).floor() as u8).min(ALPHABETS[i].1 - 1);
            longitude -= f64::from(x) * width;
            latitude -= f64::from(y) * height;
            cells[i] = [x, y];
        }
        Self { cells, precision }
    }
}

impl FromStr for Locator {
    type Err = Error;

    fn from_str(locator: &str) -> Result<Self, Self::Err> {
        Self::parse(locator)
    }
}

impl fmt::Display for Locator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, &[x, y]) in self.cells[..self.precision as usize].iter().enumerate() {
            write!(
                f,
                "{}{}",
                char::from(ALPHABETS[i].0 + x),
                char::from(ALPHABETS[i].0 + y)
            )?;
        }
        Ok(())
    }
}
