// SPDX-License-Identifier: GPL-3.0-or-later
//! Terminal palettes and capability selection (QGDXC-TUI-0071, QGDXC-TUI-0072).
use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Color,
    Daylight,
    Phosphor,
    Amber,
    Mono,
}
impl Theme {
    pub const ALL: [Self; 5] = [
        Self::Color,
        Self::Daylight,
        Self::Phosphor,
        Self::Amber,
        Self::Mono,
    ];
    pub fn name(self) -> &'static str {
        ["color", "daylight", "phosphor", "amber", "mono"][self as usize]
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == value)
    }
    pub fn initial(explicit: Option<Self>, no_color: Option<&str>) -> Self {
        explicit.unwrap_or(if no_color.is_some_and(|s| !s.is_empty()) {
            Self::Mono
        } else {
            Self::Color
        })
    }
    pub fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Background,
    Text,
    Dim,
    Border,
    Title,
    Accent,
    Good,
    Warning,
    Bad,
    Spark,
    Human,
    Skimmer,
    Spotter,
    Frequency,
    Time,
    Heading,
    New,
    LogoBackground,
    LogoForeground,
    Cw,
    Ft8,
    Ssb,
    Rtty,
    B160,
    B80,
    B60,
    B40,
    B30,
    B20,
    B17,
    B15,
    B12,
    B10,
    B6,
}
const PALETTES: [[u32; 34]; 5] = [
    [
        // color
        0x0f1316, 0xc5ced6, 0x66727d, 0x34414c, 0xeef3f7, 0xffb454, 0x7fd962, 0xffb454, 0xf07178,
        0x5ccfe6, 0xffffff, 0xc5ced6, 0x7f8b96, 0xe6edf3, 0x8f9ba6, 0x8f9ba6, 0x26445a, 0xffb454,
        0x0f1316, 0x5ccfe6, 0xd4a6ff, 0xffd580, 0x95e6cb, 0xf07178, 0xf29668, 0xffb454, 0xffd580,
        0xd5e36b, 0x91d76a, 0x5fd4a0, 0x5ccfe6, 0x59a8f0, 0x8f8cf5, 0xd4a6ff,
    ],
    [
        // daylight
        0xf7f7f2, 0x2b3036, 0x7a838c, 0xb9c1c8, 0x111417, 0xb35c00, 0x2f8a2f, 0xb35c00, 0xc0392b,
        0x0f7f9a, 0x000000, 0x2b3036, 0x6b747d, 0x111417, 0x6b747d, 0x6b747d, 0xffe2a6, 0xb35c00,
        0xffffff, 0x0f7f9a, 0x8a3fc0, 0x946500, 0x13866a, 0xc0392b, 0xc8541f, 0xb8730a, 0x9a7d00,
        0x6f8a00, 0x2f8a2f, 0x13866a, 0x0f7f9a, 0x2563c4, 0x5b4fd0, 0x8a3fc0,
    ],
    [
        // phosphor
        0x041007, 0x3fe36c, 0x1f7a3a, 0x17552a, 0xb6ffc9, 0xb6ffc9, 0xb6ffc9, 0xb6ffc9, 0xb6ffc9,
        0x3fe36c, 0xb6ffc9, 0x3fe36c, 0x2aa64c, 0x3fe36c, 0x2aa64c, 0x2aa64c, 0x0e3d1c, 0x3fe36c,
        0x041007, 0x3fe36c, 0x3fe36c, 0x3fe36c, 0x3fe36c, 0x3fe36c, 0x3fe36c, 0x3fe36c, 0x3fe36c,
        0x3fe36c, 0x3fe36c, 0x3fe36c, 0x3fe36c, 0x3fe36c, 0x3fe36c, 0x3fe36c,
    ],
    [
        // amber
        0x120a00, 0xffb000, 0x8a5d00, 0x5c3e00, 0xffd479, 0xffd479, 0xffd479, 0xffd479, 0xffd479,
        0xffb000, 0xffd479, 0xffb000, 0xc08400, 0xffb000, 0xc08400, 0xc08400, 0x3d2700, 0xffb000,
        0x120a00, 0xffb000, 0xffb000, 0xffb000, 0xffb000, 0xffb000, 0xffb000, 0xffb000, 0xffb000,
        0xffb000, 0xffb000, 0xffb000, 0xffb000, 0xffb000, 0xffb000, 0xffb000,
    ],
    [
        // mono
        0x121212, 0xd0d0d0, 0x808080, 0x5a5a5a, 0xffffff, 0xffffff, 0xffffff, 0xffffff, 0xffffff,
        0xd0d0d0, 0xffffff, 0xd0d0d0, 0x909090, 0xd0d0d0, 0x909090, 0x909090, 0x3a3a3a, 0xd0d0d0,
        0x121212, 0xd0d0d0, 0xd0d0d0, 0xd0d0d0, 0xd0d0d0, 0xd0d0d0, 0xd0d0d0, 0xd0d0d0, 0xd0d0d0,
        0xd0d0d0, 0xd0d0d0, 0xd0d0d0, 0xd0d0d0, 0xd0d0d0, 0xd0d0d0, 0xd0d0d0,
    ],
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Depth {
    #[default]
    TrueColor,
    Indexed,
    Ansi,
}
impl Depth {
    pub fn detect(colorterm: &str, term: &str) -> Self {
        if matches!(
            colorterm.to_ascii_lowercase().as_str(),
            "truecolor" | "24bit"
        ) {
            Self::TrueColor
        } else if term.contains("256color") {
            Self::Indexed
        } else {
            Self::Ansi
        }
    }
}
#[derive(Clone, Copy, Default)]
pub struct Appearance {
    pub theme: Theme,
    pub depth: Depth,
}
impl Appearance {
    pub fn color(self, role: Role) -> Color {
        if self.theme == Theme::Mono {
            return Color::Reset;
        }
        let rgb = PALETTES[self.theme as usize][role as usize];
        match self.depth {
            Depth::TrueColor => Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8),
            Depth::Indexed => Color::Indexed(nearest(rgb, 256)),
            Depth::Ansi => {
                let mut index = nearest(rgb, 16);
                if index == 0
                    && matches!(
                        role,
                        Role::Dim | Role::Border | Role::Spotter | Role::Time | Role::Heading
                    )
                {
                    index = 8;
                }
                [
                    Color::Black,
                    Color::Red,
                    Color::Green,
                    Color::Yellow,
                    Color::Blue,
                    Color::Magenta,
                    Color::Cyan,
                    Color::Gray,
                    Color::DarkGray,
                    Color::LightRed,
                    Color::LightGreen,
                    Color::LightYellow,
                    Color::LightBlue,
                    Color::LightMagenta,
                    Color::LightCyan,
                    Color::White,
                ][index as usize]
            }
        }
    }
    pub fn style(self, role: Role) -> Style {
        Style::default().fg(self.color(role))
    }
    pub fn chip(self, logo: bool) -> Style {
        if self.theme == Theme::Mono {
            return Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD);
        }
        self.style(if logo {
            Role::LogoForeground
        } else {
            Role::Background
        })
        .bg(self.color(if logo {
            Role::LogoBackground
        } else {
            Role::Warning
        }))
        .add_modifier(Modifier::BOLD)
    }
    pub fn highlight(self, age_ms: u128) -> Style {
        if age_ms >= 3000 {
            return Style::default();
        }
        if self.theme == Theme::Mono {
            return Style::default().add_modifier(if age_ms < 1000 {
                Modifier::REVERSED
            } else {
                Modifier::BOLD
            });
        }
        // Three palette-defined steps; the last returns to the terminal background.
        let rgb = PALETTES[self.theme as usize][Role::New as usize];
        let step = (3 - age_ms / 1000) as u32;
        let base = PALETTES[self.theme as usize][Role::Background as usize];
        let blend = |shift: u32| {
            ((((rgb >> shift) & 255u32) * step + ((base >> shift) & 255u32) * (3 - step)) / 3) as u8
        };
        let rgb = (u32::from(blend(16)) << 16) | (u32::from(blend(8)) << 8) | u32::from(blend(0));
        let color = match self.depth {
            Depth::TrueColor => Color::Rgb(blend(16), blend(8), blend(0)),
            Depth::Indexed => Color::Indexed(nearest(rgb, 256)),
            Depth::Ansi => return Style::default().add_modifier(Modifier::BOLD),
        };
        Style::default().bg(color)
    }
}
fn nearest(rgb: u32, limit: u16) -> u8 {
    (0..limit)
        .min_by_key(|i| {
            let value = xterm(*i);
            [0, 8, 16]
                .into_iter()
                .map(|shift| {
                    let d = ((rgb >> shift) & 255) as i32 - ((value >> shift) & 255) as i32;
                    d * d
                })
                .sum::<i32>()
        })
        .unwrap_or(0) as u8
}
fn xterm(i: u16) -> u32 {
    const ANSI: [u32; 16] = [
        0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xc0c0c0, 0x808080,
        0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff,
    ];
    if i < 16 {
        ANSI[i as usize]
    } else if i < 232 {
        let i = i - 16;
        let levels = [0, 95, 135, 175, 215, 255];
        (levels[(i / 36) as usize] << 16)
            | (levels[((i / 6) % 6) as usize] << 8)
            | levels[(i % 6) as usize]
    } else {
        u32::from(8 + (i - 232) * 10) * 0x010101
    }
}
