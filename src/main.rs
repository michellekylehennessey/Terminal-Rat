// terminal-rat/src/main.rs
// A cute terminal pet rat you can "pet" with the mouse or by pressing 'p'.
// Built with ratatui (TUI), crossterm (input), and rodio (audio squeaks).
//
// Controls:
//  - Left-click on the rat (or press 'p' / space / enter) to pet -> rat squeaks + happiness increases
//  - 'q' or Esc to quit
//
// Notes:
//  - This demo generates squeaks procedurally (no sound files needed)
//  - Requires a working audio output device

use std::{
    error::Error,
    io,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{BarChart, Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};

use rodio::{source::Source, OutputStream, Sink};

/// Generate a short squeak sound
fn play_squeak(sink: &Sink, duration_ms: u64) {
    let total = Duration::from_millis(duration_ms);
    let steps = 6;
    let base_freq = 1600.0;
    let freq_step = 450.0;
    let seg = total / steps as u32;

    for i in 0..steps {
        let f = base_freq + i as f32 * freq_step as f32;
        let vol = 0.25 + 0.12 * (i as f32);
        let wave = rodio::source::SineWave::new(f)
            .take_duration(seg)
            .amplify(vol as f32)
            .fade_in(Duration::from_millis(8));
        sink.append(wave);
    }
}

/// Different ASCII rat art styles
#[derive(Clone, Copy)]
enum RatStyle {
    Classic,
    LongTail,
    Chubby,
}

/// App state
#[derive(Clone)]
struct App {
    last_pet: Instant,
    happiness: f32,
    vibe: f32,
    squeaks: usize,
    rat_area: Rect,
    style: RatStyle,
}

impl App {
    fn new() -> Self {
        Self {
            last_pet: Instant::now(),
            happiness: 0.5,
            vibe: 0.0,
            squeaks: 0,
            rat_area: Rect::new(0, 0, 0, 0),
            style: RatStyle::Classic,
        }
    }

    fn pet(&mut self) {
        self.happiness = (self.happiness + 0.08).clamp(0.0, 1.0);
        self.last_pet = Instant::now();
        self.squeaks += 1;
    }

    fn tick(&mut self, dt: f32) {
        self.vibe = (self.vibe + dt * 0.9) % 1.0;
        self.happiness = (self.happiness - dt * 0.015).clamp(0.0, 1.0);
    }
}

/// Pad ASCII block lines to equal width
fn pad_block(lines: Vec<String>) -> Vec<String> {
    let width = lines.iter().map(|s| s.chars().count()).max().unwrap_or(0);
    lines
        .into_iter()
        .map(|s| {
            let pad = width.saturating_sub(s.chars().count());
            format!("{s}{}", " ".repeat(pad))
        })
        .collect()
}

/// Return ASCII art for a given rat style
fn rat_art(vibe: f32, happy: f32, style: RatStyle) -> Vec<String> {
    match style {
        RatStyle::Classic => {
            let tail = if vibe < 0.5 { "~" } else { "≈" };
            let eye = if happy > 0.66 { "•" } else { "." };
            let blush = if happy > 0.66 { "˘" } else { " " };
            pad_block(vec![
                format!("  (\\_/)     {tail}{tail}{tail}"),
                format!("  ({eye}{blush}{eye})     "),
                "  (   )    ".to_string(),
                "  (   )    ".to_string(),
                "   \" \"     ".to_string(),
            ])
        }
        RatStyle::LongTail => {
            let tail = if vibe < 0.5 { "~~" } else { "≈≈" };
            let eye = if happy > 0.5 { "•" } else { "." };
            pad_block(vec![
                format!("  (\\_/)      {tail}{tail}{tail}{tail}"),
                format!("  ({eye} .)    "),
                "  (   )    ".to_string(),
                "   v v     ".to_string(),
            ])
        }
        RatStyle::Chubby => {
            let tail = if vibe < 0.5 { "~" } else { "≈" };
            let eye = if happy > 0.7 { "•" } else { "o" };
            pad_block(vec![
                format!("  (\\_/)    {tail}{tail}{tail}"),
                format!(" ( {eye} {eye} ) "),
                " (  -  ) ".to_string(),
                " (     ) ".to_string(),
                "  \"   \"  ".to_string(),
            ])
        }
    }
}

fn draw_ui(frame: &mut Frame, app: &mut App) {
    let size = frame.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(6), Constraint::Length(3)])
        .split(size);

    let header = Paragraph::new(Line::from(vec![
        Span::styled("terminal-rat ", Style::default().fg(Color::Magenta)),
        Span::raw("— click or press 'p' to pet, 's' to switch skins, 'q' to quit."),
    ]))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Squeak Guide"));
    frame.render_widget(header, chunks[0]);

    let center = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    let rat_block = Block::default().borders(Borders::ALL).title("Your Rat");

    let art = rat_art(app.vibe, app.happiness, app.style);
    let rat_text = art
        .iter()
        .map(|l| Line::from(Span::raw(l.clone())))
        .collect::<Vec<_>>();

    let rat_para = Paragraph::new(rat_text)
        .alignment(Alignment::Center) // now works with padding
        .wrap(Wrap { trim: false })
        .block(rat_block);

    app.rat_area = center[0];
    frame.render_widget(rat_para, center[0]);

    let happiness_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Happiness"))
        .gauge_style(Style::default().fg(Color::Green))
        .ratio(app.happiness as f64)
        .label(format!("{:.0}%", app.happiness * 100.0));

    let bars: Vec<(&str, u64)> = (0..8)
        .map(|i| {
            let v = (((app.vibe * 8.0) as i32 - i as i32).abs() as u64) % 4 + 1;
            (" ", v)
        })
        .collect();
    let barchart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title("Energy"))
        .data(&bars)
        .bar_width(2)
        .bar_gap(1)
        .value_style(Style::default().fg(Color::Black).bg(Color::White));

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(7), Constraint::Min(3)])
        .split(center[1]);

    let squeak_info = Paragraph::new(Line::from(vec![
        Span::raw("Squeaks so far: "),
        Span::styled(format!("{}", app.squeaks), Style::default().fg(Color::Yellow)),
        Span::raw("   (pet to squeak)"),
    ]))
        .block(Block::default().borders(Borders::ALL).title("Stats"));

    frame.render_widget(happiness_gauge, right[0]);
    frame.render_widget(barchart, right[1]);
    frame.render_widget(squeak_info, right[2]);

    let footer = Paragraph::new(Line::from(vec![
        Span::raw("Made with ratatui + crossterm + rodio. "),
        Span::styled("Squeak!", Style::default().fg(Color::LightMagenta)),
    ]))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("About"));

    frame.render_widget(footer, chunks[2]);
}

fn in_rat_bounds(app: &App, mouse_x: u16, mouse_y: u16) -> bool {
    app.rat_area.contains(Position { x: mouse_x, y: mouse_y })
}

fn main() -> Result<(), Box<dyn Error>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Arc::new(Mutex::new(Sink::try_new(&stream_handle)?));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_state = Arc::new(Mutex::new(App::new()));
    let tick_rate = Duration::from_millis(33);
    let mut last = Instant::now();

    let res = loop {
        terminal.draw(|f| {
            let mut app = app_state.lock().unwrap();
            draw_ui(f, &mut app);
        })?;

        let now = Instant::now();
        let dt = (now - last).as_secs_f32();
        last = now;
        {
            let mut app = app_state.lock().unwrap();
            app.tick(dt);
        }

        if event::poll(Duration::from_millis(1))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                    KeyCode::Char('p') | KeyCode::Enter | KeyCode::Char(' ') => {
                        let mut app = app_state.lock().unwrap();
                        app.pet();
                        let sink = Arc::clone(&sink);
                        thread::spawn(move || {
                            if let Ok(s) = sink.lock() {
                                play_squeak(&*s, 140);
                                s.sleep_until_end();
                            }
                        });
                    }
                    KeyCode::Char('s') => {
                        let mut app = app_state.lock().unwrap();
                        app.style = match app.style {
                            RatStyle::Classic => RatStyle::LongTail,
                            RatStyle::LongTail => RatStyle::Chubby,
                            RatStyle::Chubby => RatStyle::Classic,
                        };
                    }
                    _ => {}
                },
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        let (x, y) = (m.column, m.row);
                        let mut app = app_state.lock().unwrap();
                        if in_rat_bounds(&app, x, y) {
                            app.pet();
                            let sink = Arc::clone(&sink);
                            thread::spawn(move || {
                                if let Ok(s) = sink.lock() {
                                    play_squeak(&*s, 140);
                                    s.sleep_until_end();
                                }
                            });
                        }
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        thread::sleep(tick_rate);
    };

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}


