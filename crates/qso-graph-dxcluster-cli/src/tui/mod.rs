// SPDX-License-Identifier: GPL-3.0-or-later
mod config;
mod model;
mod render;
pub(crate) mod sources;
#[cfg(test)]
mod tests;
use config::Options;
use crossterm::{
    cursor::{RestorePosition, SavePosition},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use model::{Input, Model, sanitize};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};
use sources::Workers;
use std::{
    io::{self, IsTerminal, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

// QGDXC-TUI-0052: keep the saved terminal state in a guard established before
// the first mutation. Rendering never changes cursor visibility, so both an
// initially hidden cursor and an initially visible cursor remain as they were.
trait ScreenIo {
    fn raw(&mut self) -> io::Result<bool>;
    fn set_raw(&mut self, enabled: bool) -> io::Result<()>;
    fn enter_screen(&mut self) -> io::Result<()>;
    fn leave_screen(&mut self) -> io::Result<()>;
    fn enter_paste(&mut self) -> io::Result<()>;
    fn leave_paste(&mut self) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
}
struct SystemScreen;
impl ScreenIo for SystemScreen {
    fn raw(&mut self) -> io::Result<bool> {
        terminal::is_raw_mode_enabled()
    }
    fn set_raw(&mut self, enabled: bool) -> io::Result<()> {
        if enabled {
            terminal::enable_raw_mode()
        } else {
            terminal::disable_raw_mode()
        }
    }
    fn enter_screen(&mut self) -> io::Result<()> {
        execute!(io::stdout(), SavePosition, EnterAlternateScreen)
    }
    fn leave_screen(&mut self) -> io::Result<()> {
        execute!(io::stdout(), LeaveAlternateScreen, RestorePosition)
    }
    fn enter_paste(&mut self) -> io::Result<()> {
        // XTSAVE/XTRESTORE preserve the caller's bracketed-paste mode.
        io::stdout().write_all(b"\x1b[?2004s\x1b[?2004h")
    }
    fn leave_paste(&mut self) -> io::Result<()> {
        io::stdout().write_all(b"\x1b[?2004r")
    }
    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}
struct Screen<T: ScreenIo> {
    io: T,
    raw_changed: bool,
    alternate: bool,
    paste: bool,
}
impl<T: ScreenIo> Screen<T> {
    fn enter(io: T) -> io::Result<Self> {
        let mut screen = Self {
            io,
            raw_changed: false,
            alternate: false,
            paste: false,
        };
        if !screen.io.raw()? {
            screen.raw_changed = true;
            screen.io.set_raw(true)?;
        }
        // Set flags before fallible writes, which can succeed partially.
        screen.alternate = true;
        screen.io.enter_screen()?;
        screen.paste = true;
        screen.io.enter_paste()?;
        screen.io.flush()?;
        Ok(screen)
    }
}
impl<T: ScreenIo> Drop for Screen<T> {
    fn drop(&mut self) {
        if self.paste {
            let _ = self.io.leave_paste();
        }
        if self.alternate {
            let _ = self.io.leave_screen();
        }
        if self.raw_changed {
            let _ = self.io.set_raw(false);
        }
        let _ = self.io.flush();
    }
}
pub fn input(event: Event) -> Option<Input> {
    match event {
        Event::Paste(s) => Some(Input::Paste(s)),
        Event::Key(key) if key.kind != KeyEventKind::Release => {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c' | 'C'))
            {
                return Some(Input::CtrlC);
            }
            Some(match key.code {
                KeyCode::Char(c) => Input::Char(c),
                KeyCode::Backspace => Input::Backspace,
                KeyCode::Enter => Input::Enter,
                KeyCode::Esc => Input::Escape,
                KeyCode::Up => Input::Up,
                KeyCode::Down => Input::Down,
                KeyCode::PageUp => Input::PageUp,
                KeyCode::PageDown => Input::PageDown,
                KeyCode::Home => Input::Home,
                _ => return None,
            })
        }
        _ => None,
    }
}
// Manual frame presentation preserves cursor visibility (the default draw method
// would hide it). The same renderer is exercised with TestBackend in unit tests.
fn present<B: Backend>(
    terminal: &mut Terminal<B>,
    model: &Model,
    now: Duration,
) -> Result<(), B::Error> {
    terminal.autoresize()?;
    render::draw(&mut terminal.get_frame(), model, now);
    terminal.flush()?;
    terminal.swap_buffers();
    terminal.backend_mut().flush()
}
#[derive(Default)]
struct Redraw {
    last: Option<Duration>,
}
impl Redraw {
    fn due(&mut self, now: Duration) -> bool {
        if self
            .last
            .is_none_or(|last| now.saturating_sub(last) >= Duration::from_millis(50))
        {
            self.last = Some(now);
            true
        } else {
            false
        }
    }
}
pub fn run(args: &[String]) -> Result<u8, String> {
    let options = Options::parse(args).map_err(|e| sanitize(&e))?;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("tui requires terminal standard input and output".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))
        .map_err(|e| sanitize(&e.to_string()))?;
    // Defer panic diagnostics until every guard has restored the terminal.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> io::Result<u8> {
        let _screen = Screen::enter(SystemScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        let origin = Instant::now();
        let mut model = Model::new(options.labels.clone());
        let mut workers = Workers::start(options, origin, stop.clone());
        let mut redraw = Redraw::default();
        while !model.quit && !stop.load(Ordering::Relaxed) {
            workers.drain(&mut model, origin);
            let now = origin.elapsed();
            if redraw.due(now) {
                model.page = terminal
                    .size()?
                    .height
                    .saturating_sub(model.sources.len() as u16 + 5)
                    .max(1) as usize;
                present(&mut terminal, &model, now)?;
            }
            if event::poll(Duration::from_millis(5))?
                && let Some(input) = input(event::read()?)
            {
                let size = terminal.size()?;
                model.key_at_size(input, size.width, size.height);
            }
        }
        Ok(if stop.load(Ordering::Relaxed) { 130 } else { 0 })
    }));
    std::panic::set_hook(hook);
    match result {
        Ok(result) => result.map_err(|e| sanitize(&e.to_string())),
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic");
            Err(format!("interface panic: {}", sanitize(message)))
        }
    }
}
