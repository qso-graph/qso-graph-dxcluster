// SPDX-License-Identifier: GPL-3.0-or-later
mod beacons;
mod config;
mod geo;
mod model;
mod picker;
mod render;
pub(crate) mod sources;
#[cfg(test)]
mod tests;
mod theme;
mod weather;
use config::Options;
use crossterm::{
    cursor::{Hide, RestorePosition, SavePosition, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use model::{Input, Model, SpotsView, sanitize};
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
// the first mutation. The cursor is hidden while the interface owns the screen
// and shown on exit: a terminal cannot report its earlier cursor visibility.
trait ScreenIo {
    fn raw(&mut self) -> io::Result<bool>;
    fn set_raw(&mut self, enabled: bool) -> io::Result<()>;
    fn enter_screen(&mut self) -> io::Result<()>;
    fn leave_screen(&mut self) -> io::Result<()>;
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
        execute!(io::stdout(), SavePosition, EnterAlternateScreen, Hide)
    }
    fn leave_screen(&mut self) -> io::Result<()> {
        execute!(io::stdout(), Show, LeaveAlternateScreen, RestorePosition)
    }
    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}
struct Screen<T: ScreenIo> {
    io: T,
    raw_changed: bool,
    alternate: bool,
}
impl<T: ScreenIo> Screen<T> {
    fn enter(io: T) -> io::Result<Self> {
        let mut screen = Self {
            io,
            raw_changed: false,
            alternate: false,
        };
        if !screen.io.raw()? {
            screen.raw_changed = true;
            screen.io.set_raw(true)?;
        }
        // Set flags before fallible writes, which can succeed partially.
        screen.alternate = true;
        screen.io.enter_screen()?;
        screen.io.flush()?;
        Ok(screen)
    }
}
impl<T: ScreenIo> Drop for Screen<T> {
    fn drop(&mut self) {
        if self.alternate {
            let _ = self.io.leave_screen();
        }
        if self.raw_changed {
            let _ = self.io.set_raw(false);
        }
        let _ = self.io.flush();
    }
}
// Field order is the cleanup order, including during unwinding (QGDXC-TUI-0052).
struct Interface<T: ScreenIo> {
    _screen: Screen<T>,
    workers: Workers,
}
pub fn input(event: Event) -> Option<Input> {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c' | 'C'))
            {
                return Some(Input::CtrlC);
            }
            Some(match key.code {
                KeyCode::Char(c) => Input::Char(c),
                KeyCode::Backspace => Input::Backspace,
                KeyCode::Tab => Input::Tab,
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
// Manual frame presentation leaves cursor visibility to the screen guard. The same renderer is exercised with TestBackend in unit tests.
fn present<B: Backend>(
    terminal: &mut Terminal<B>,
    model: &Model,
    now: Duration,
) -> Result<bool, B::Error> {
    terminal.autoresize()?;
    let mut frame = terminal.get_frame();
    let flux_chart_drawn = render::draw(&mut frame, model, now);
    terminal.flush()?;
    terminal.swap_buffers();
    terminal.backend_mut().flush()?;
    Ok(flux_chart_drawn)
}
fn present_picker<B: Backend>(
    terminal: &mut Terminal<B>,
    chooser: &picker::Picker,
    appearance: theme::Appearance,
) -> Result<(), B::Error> {
    terminal.autoresize()?;
    picker::draw(&mut terminal.get_frame(), chooser, appearance);
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
    let mut options = match Options::parse(args) {
        Ok(options) => options,
        // QGDXC-TUI-0006: a first run gets the way forward, not only the error. The
        // path is already escaped; the hint lines are this program's own text.
        Err(e) if e.starts_with(config::NO_SOURCES_FILE) => {
            eprintln!("qg-dxc: {}", sanitize(&e));
            eprintln!("{}", config::FIRST_RUN);
            return Ok(1);
        }
        Err(e) => return Err(sanitize(&e)),
    };
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
        let screen = Screen::enter(SystemScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        let appearance = theme::Appearance {
            theme: options.theme,
            depth: theme::Depth::detect(
                &std::env::var("COLORTERM").unwrap_or_default(),
                &std::env::var("TERM").unwrap_or_default(),
            ),
        };
        if let Some(saved) = &options.saved {
            // QGDXC-TUI-0008: nothing opens until the operator chooses.
            let mut chooser = picker::Picker::new(saved);
            let mut redraw = Redraw::default();
            let start = Instant::now();
            loop {
                if stop.load(Ordering::Relaxed) {
                    return Ok(130);
                }
                if redraw.due(start.elapsed()) {
                    present_picker(&mut terminal, &chooser, appearance)?;
                }
                if event::poll(Duration::from_millis(5))?
                    && let Some(input) = input(event::read()?)
                {
                    redraw = Redraw::default();
                    match chooser.key(input) {
                        picker::Outcome::Stay => {}
                        picker::Outcome::Quit => return Ok(0),
                        picker::Outcome::Start(chosen) => {
                            options.choose(&chosen);
                            break;
                        }
                    }
                }
            }
            terminal.clear()?;
        }
        let origin = Instant::now();
        let mut model = Model::new(
            options.labels.clone(),
            options.keep_human,
            options.keep_skimmer,
        );
        model.appearance = appearance;
        if options.space_weather {
            model.spots_view = SpotsView::Weather;
        }
        model.utc_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut interface = Interface {
            _screen: screen,
            workers: Workers::new(stop.clone()),
        };
        interface.workers.start(options, origin);
        let mut weather = weather::Worker::default();
        let mut redraw = Redraw::default();
        while !model.quit && !stop.load(Ordering::Relaxed) {
            interface.workers.drain(&mut model, origin);
            let now = origin.elapsed();
            let flux_chart_drawn = if redraw.due(now) {
                let size = terminal.size()?;
                model.page =
                    render::list_rows(size.width, size.height, model.sources.len()).max(1) as usize;
                present(&mut terminal, &model, now)?
            } else {
                false
            };
            weather.poll(
                model.spots_view == SpotsView::Weather,
                flux_chart_drawn,
                |update| model.apply_weather(update),
                || weather::start(origin),
            )?;
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
