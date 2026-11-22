// SPDX-License-Identifier: GPL-3.0-or-later
//! Alert message metadata and effective periods (QGDXC-SWX-0009–0011).

use super::utc;
use std::{cmp::Reverse, ops::Range};

const DAY: i64 = 86_400;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlertKind {
    Alert,
    ContinuedAlert,
    Warning,
    ExtendedWarning,
    Watch,
    CancelWarning,
    CancelWatch,
    Summary,
}

impl AlertKind {
    fn from_label(label: &str) -> Option<Self> {
        Some(match label {
            "ALERT" => Self::Alert,
            "CONTINUED ALERT" => Self::ContinuedAlert,
            "WARNING" => Self::Warning,
            "EXTENDED WARNING" => Self::ExtendedWarning,
            "WATCH" => Self::Watch,
            "CANCEL WARNING" => Self::CancelWarning,
            "CANCEL WATCH" => Self::CancelWatch,
            "SUMMARY" => Self::Summary,
            _ => return None,
        })
    }

    fn severity(self) -> Option<u8> {
        match self {
            Self::Alert | Self::ContinuedAlert => Some(0),
            Self::Warning | Self::ExtendedWarning => Some(1),
            Self::Watch => Some(2),
            Self::CancelWarning | Self::CancelWatch | Self::Summary => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlertFamily {
    Geomagnetic,
    RadioBlackout,
    SolarRadiation,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchDay {
    /// Midnight UTC in seconds since the Unix epoch.
    pub date: i64,
    /// The level token from the forecast, such as `None` or `G1`.
    pub level: String,
}

/// Alert timestamps are UTC seconds since the Unix epoch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Alert {
    pub product_id: String,
    /// Timestamp from the JSON envelope (QGDXC-SWX-0003).
    pub issue_time: i64,
    /// Escape this text before display.
    pub headline: Option<String>,
    pub kind: Option<AlertKind>,
    pub serial: Option<u64>,
    /// Timestamp from the message's Issue Time line (QGDXC-SWX-0009).
    pub message_issue_time: Option<i64>,
    pub valid_from: Option<i64>,
    pub valid_to: Option<i64>,
    pub valid_until: Option<i64>,
    pub extended_serial: Option<u64>,
    pub cancelled_serial: Option<u64>,
    pub watch_days: Option<Vec<WatchDay>>,
}

impl Alert {
    pub(super) fn from_message(id: &str, issue_time: i64, message: &str) -> Self {
        let field = |label| {
            message.lines().find_map(|line| {
                let (key, value) = line.trim_start_matches(' ').split_once(':')?;
                (key == label).then_some(value.trim_ascii())
            })
        };
        let time = |label| utc::message_time(field(label)?).map(|(time, _, _)| time);
        let serial = |label| -> Option<u64> {
            let text = field(label)?;
            if !text.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            text.parse().ok()
        };
        let (headline, kind) = message
            .lines()
            .find_map(|line| {
                let (label, _) = line.trim_start_matches(' ').split_once(':')?;
                let kind = AlertKind::from_label(label)?;
                Some((Some(line.to_owned()), Some(kind)))
            })
            .unwrap_or_default();
        let issue = field("Issue Time").and_then(utc::message_time);
        let watch_days = issue
            .filter(|_| kind == Some(AlertKind::Watch))
            .and_then(|(_, year, month)| watch_days(message, year, month));
        Self {
            product_id: id.to_owned(),
            issue_time,
            headline,
            kind,
            serial: serial("Serial Number"),
            message_issue_time: issue.map(|(time, _, _)| time),
            valid_from: time("Valid From"),
            valid_to: time("Valid To"),
            valid_until: time("Now Valid Until"),
            extended_serial: serial("Extension to Serial Number"),
            cancelled_serial: serial("Cancel Serial Number"),
            watch_days,
        }
    }

    /// Classify the headline (QGDXC-SWX-0010).
    pub fn family(&self) -> AlertFamily {
        let Some(headline) = &self.headline else {
            return AlertFamily::Other;
        };
        let headline = headline.to_ascii_lowercase();
        if headline.contains("geomagnetic") {
            AlertFamily::Geomagnetic
        } else if headline.contains("x-ray") || headline.contains("radio blackout") {
            AlertFamily::RadioBlackout
        } else if headline.contains("proton") || headline.contains("solar radiation") {
            AlertFamily::SolarRadiation
        } else {
            AlertFamily::Other
        }
    }

    fn period(&self) -> Option<Range<i64>> {
        use AlertKind::*;
        let period = match self.kind? {
            Alert | ContinuedAlert => {
                let start = self.message_issue_time?;
                start..start.checked_add(3 * 3600)?
            }
            Warning => self.valid_from?..self.valid_to?,
            ExtendedWarning => self.message_issue_time?..self.valid_until?,
            Watch => {
                let last = self
                    .watch_days
                    .as_ref()?
                    .iter()
                    .filter(|day| day.level != "None")
                    .map(|day| day.date)
                    .max()?;
                self.message_issue_time?..last.checked_add(DAY)?
            }
            CancelWarning | CancelWatch | Summary => return None,
        };
        (!period.is_empty()).then_some(period)
    }

    /// The scheduled end for display (QGDXC-SWX-0011).
    pub fn end_time(&self) -> Option<i64> {
        let end = self.period()?.end;
        Some(end - i64::from(self.kind == Some(AlertKind::Watch)))
    }

    fn ends(&self, item: &Self, t: i64) -> bool {
        use AlertKind::*;
        let (Some(serial), Some(issue)) = (item.serial, self.message_issue_time) else {
            return false;
        };
        if issue > t {
            return false;
        }
        match (self.kind, item.kind) {
            (Some(ExtendedWarning), Some(Warning)) => {
                self.extended_serial == Some(serial) && self.period().is_some()
            }
            (Some(CancelWarning), Some(Warning | ExtendedWarning))
            | (Some(CancelWatch), Some(Watch)) => self.cancelled_serial == Some(serial),
            _ => false,
        }
    }
}

fn watch_days(message: &str, mut year: i64, mut previous_month: i64) -> Option<Vec<WatchDay>> {
    let mut lines = message.lines();
    lines.find(|line| line.trim_ascii() == "Highest Storm Level Predicted by Day:")?;
    let line = lines.next()?;
    let mut fields = line.split_ascii_whitespace().peekable();
    let mut days = Vec::new();
    while let Some(month) = fields.next() {
        if days.len() == 3 {
            return None;
        }
        let month = utc::month(month)?;
        let day = utc::day(fields.next()?.strip_suffix(':')?)?;
        let level = fields.next()?;
        if level != "None" && !matches!(level.as_bytes(), [b'G' | b'R' | b'S', b'1'..=b'5']) {
            return None;
        }
        if let Some(note) = fields.next_if(|text| text.starts_with('(')) {
            let mut word = note.strip_prefix('(')?;
            // The NOAA forecast can append a parenthesized description to a level.
            while !word.ends_with(')') {
                word = fields.next()?;
                if word.contains(['(', ':']) {
                    return None;
                }
            }
        }
        if previous_month == 12 && month == 1 {
            year += 1;
        }
        days.push(WatchDay {
            date: utc::midnight(year, month, day)?,
            level: level.to_owned(),
        });
        previous_month = month;
    }
    (!days.is_empty()).then_some(days)
}

/// Return the items in force at UTC second `t` (QGDXC-SWX-0011).
pub fn in_force(alerts: &[Alert], t: i64) -> Vec<&Alert> {
    let mut ordered: Vec<_> = alerts
        .iter()
        .filter_map(|item| {
            let severity = item.kind?.severity()?;
            let family = item.family();
            (family != AlertFamily::Other).then_some((severity, family, item))
        })
        .collect();
    ordered.sort_by_key(|&(severity, _, item)| (severity, Reverse(item.message_issue_time)));

    let mut watch_families = Vec::new();
    ordered
        .into_iter()
        .filter_map(|(_, family, item)| {
            if item.kind == Some(AlertKind::Watch) {
                if item.message_issue_time? > t || watch_families.contains(&family) {
                    return None;
                }
                watch_families.push(family);
            }
            let active = item.period()?.contains(&t);
            (active && !alerts.iter().any(|notice| notice.ends(item, t))).then_some(item)
        })
        .collect()
}
