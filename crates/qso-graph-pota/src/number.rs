// SPDX-License-Identifier: GPL-3.0-or-later
//! Keep decimal precision for integer fields and range boundaries.

pub(super) struct Decimal {
    digits: Vec<u8>,
    point: i64,
    negative: bool,
}

impl Decimal {
    pub(super) fn parse(raw: &str) -> Option<Self> {
        // The input has already passed JSON syntax validation.
        let (unsigned, negative) = raw.strip_prefix('-').map_or((raw, false), |s| (s, true));
        if !unsigned.as_bytes().first()?.is_ascii_digit() {
            return None;
        }
        let (mantissa, exponent) = unsigned.split_once(['e', 'E']).unwrap_or((unsigned, "0"));
        let exponent = exponent.parse::<i64>().unwrap_or_else(|_| {
            if exponent.starts_with('-') {
                i64::MIN
            } else {
                i64::MAX
            }
        });
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let digits: Vec<_> = whole
            .bytes()
            .chain(fraction.bytes())
            .skip_while(|&b| b == b'0')
            .collect();
        let leading_zeros = whole.len() + fraction.len() - digits.len();
        let point = exponent.saturating_add(whole.len() as i64 - leading_zeros as i64);
        Some(Self {
            digits,
            point,
            negative,
        })
    }

    // The magnitude's integer part and whether there are nonzero fractional digits.
    fn parts(&self) -> Option<(u64, bool)> {
        if self.digits.is_empty() {
            return Some((0, false));
        }
        if self.point <= 0 {
            return Some((0, true));
        }
        if self.point > 20 {
            return None;
        }
        let point = self.point as usize;
        let whole = self
            .digits
            .iter()
            .copied()
            .chain(std::iter::repeat(b'0'))
            .take(point)
            .try_fold(0_u64, |n, digit| {
                n.checked_mul(10)?.checked_add(u64::from(digit - b'0'))
            })?;
        let fraction = self.digits.iter().skip(point).any(|&b| b != b'0');
        Some((whole, fraction))
    }

    pub(super) fn whole(self) -> Option<u64> {
        let (whole, fraction) = self.parts()?;
        (!fraction && (!self.negative || whole == 0)).then_some(whole)
    }
}
