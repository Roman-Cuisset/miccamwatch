use crate::{
    cli::Filter,
    i18n::Language,
    model::{Access, Activity, Device, Resource, Risk},
    platform::PlatformMonitor,
};
use anyhow::{Context, Result};
use chrono::Local;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};
use std::{
    io,
    time::{Duration, Instant},
};

pub fn run_tui(monitor: PlatformMonitor, lang: Language) -> Result<()> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to initialize ratatui terminal")?;

    let res = tui_loop(&mut terminal, monitor, lang);

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    res
}

struct TuiState {
    monitor: PlatformMonitor,
    lang: Language,
    selected_access: usize,
    table_state: TableState,
    events_log: Vec<(String, String, Color)>,
    status_msg: Option<(String, Instant, Color)>,
    muted: bool,
    devices: Vec<Device>,
}

fn tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    monitor: PlatformMonitor,
    lang: Language,
) -> Result<()> {
    let devices = monitor.devices().unwrap_or_default();
    let muted = monitor.get_microphone_mute().unwrap_or(false);

    let mut state = TuiState {
        monitor,
        lang,
        selected_access: 0,
        table_state: TableState::default(),
        events_log: Vec::new(),
        status_msg: None,
        muted,
        devices,
    };

    let filter = Filter::default();
    let mut last_poll = Instant::now() - Duration::from_secs(1);
    let mut current_accesses: Vec<Access> = Vec::new();

    loop {
        // Poll state every 250ms
        if last_poll.elapsed() >= Duration::from_millis(250) {
            last_poll = Instant::now();
            if let Ok(snapshot) = state.monitor.snapshot(&filter) {
                // Check new events
                for access in &snapshot.accesses {
                    if !current_accesses.iter().any(|a| a.key == access.key) {
                        let time = Local::now().format("%H:%M:%S").to_string();
                        let text = format!("START {} ({})", access.application, access.resource);
                        state.events_log.push((time, text, Color::Green));
                        if state.events_log.len() > 100 {
                            state.events_log.remove(0);
                        }
                    }
                }
                for prev in &current_accesses {
                    if !snapshot.accesses.iter().any(|a| a.key == prev.key) {
                        let time = Local::now().format("%H:%M:%S").to_string();
                        let text = format!("STOP  {} ({})", prev.application, prev.resource);
                        state.events_log.push((time, text, Color::DarkGray));
                        if state.events_log.len() > 100 {
                            state.events_log.remove(0);
                        }
                    }
                }
                current_accesses = snapshot.accesses;
            }
            state.muted = state.monitor.get_microphone_mute().unwrap_or(state.muted);
        }

        // Draw UI
        terminal.draw(|f| draw_ui(f, &mut state, &current_accesses))?;

        // Handle inputs with 50ms timeout
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('m') => {
                    if let Ok(new_mute) = state.monitor.toggle_microphone_mute() {
                        state.muted = new_mute;
                        let msg = if new_mute {
                            ("Microphone MUTED".to_owned(), Instant::now(), Color::Red)
                        } else {
                            (
                                "Microphone UNMUTED".to_owned(),
                                Instant::now(),
                                Color::Green,
                            )
                        };
                        state.status_msg = Some(msg);
                    }
                }
                KeyCode::Char('k') => {
                    if !current_accesses.is_empty()
                        && state.selected_access < current_accesses.len()
                    {
                        let target = &current_accesses[state.selected_access];
                        if let Some(pid) = target.pid {
                            match crate::platform::terminate_process_by_pid(pid) {
                                Ok(()) => {
                                    state.status_msg = Some((
                                        format!(
                                            "Terminated process {} (PID {})",
                                            target.application, pid
                                        ),
                                        Instant::now(),
                                        Color::Red,
                                    ));
                                }
                                Err(e) => {
                                    state.status_msg = Some((
                                        format!("Failed to kill PID {pid}: {e}"),
                                        Instant::now(),
                                        Color::Yellow,
                                    ));
                                }
                            }
                        }
                    }
                }
                KeyCode::Up => {
                    if state.selected_access > 0 {
                        state.selected_access -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !current_accesses.is_empty()
                        && state.selected_access + 1 < current_accesses.len()
                    {
                        state.selected_access += 1;
                    }
                }
                KeyCode::Char('r') => {
                    last_poll = Instant::now() - Duration::from_secs(1);
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn draw_ui(f: &mut Frame, state: &mut TuiState, accesses: &[Access]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(7), // Top panels (Devices & Stats)
            Constraint::Min(8),    // Active Accesses
            Constraint::Length(7), // Event Log
            Constraint::Length(1), // Footer shortcuts
        ])
        .split(f.area());

    // 1. Header
    let time_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mute_badge = if state.muted {
        Span::styled(
            " [MIC MUTED] ",
            Style::default().bg(Color::Red).fg(Color::White).bold(),
        )
    } else {
        Span::styled(
            " [MIC ON] ",
            Style::default().bg(Color::Green).fg(Color::Black).bold(),
        )
    };

    let title_line = Line::from(vec![
        Span::styled(" miccamwatch ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("v0.9.0 ", Style::default().fg(Color::DarkGray)),
        Span::raw("— "),
        Span::styled(time_str, Style::default().fg(Color::White)),
        Span::raw("   "),
        mute_badge,
    ]);

    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let header_para = Paragraph::new(title_line).block(header_block);
    f.render_widget(header_para, chunks[0]);

    // 2. Top panels
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    // Devices & Health
    let mut dev_lines = Vec::new();
    for dev in state.devices.iter().take(4) {
        let tag = match dev.resource {
            Resource::Microphone => {
                Span::styled("MIC ", Style::default().fg(Color::Magenta).bold())
            }
            Resource::Camera => Span::styled("CAM ", Style::default().fg(Color::Cyan).bold()),
        };
        dev_lines.push(Line::from(vec![
            tag,
            Span::styled(&dev.name, Style::default().fg(Color::White)),
        ]));
    }
    if dev_lines.is_empty() {
        dev_lines.push(Line::from(Span::styled(
            "No capture devices found",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let dev_block = Block::default()
        .title(" Devices ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    f.render_widget(Paragraph::new(dev_lines).block(dev_block), top_chunks[0]);

    // Stats & Status Message
    let active_mics = accesses
        .iter()
        .filter(|a| a.resource == Resource::Microphone)
        .count();
    let active_cams = accesses
        .iter()
        .filter(|a| a.resource == Resource::Camera)
        .count();
    let suspicious_count = accesses
        .iter()
        .filter(|a| a.risk >= Risk::Suspicious)
        .count();

    let mut stat_lines = vec![
        Line::from(vec![
            Span::raw("Active Microphones: "),
            Span::styled(
                active_mics.to_string(),
                Style::default()
                    .fg(if active_mics > 0 {
                        Color::Green
                    } else {
                        Color::DarkGray
                    })
                    .bold(),
            ),
            Span::raw("   Active Cameras: "),
            Span::styled(
                active_cams.to_string(),
                Style::default()
                    .fg(if active_cams > 0 {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    })
                    .bold(),
            ),
        ]),
        Line::from(vec![
            Span::raw("Risk alerts (Suspicious/Blocked): "),
            Span::styled(
                suspicious_count.to_string(),
                Style::default()
                    .fg(if suspicious_count > 0 {
                        Color::Red
                    } else {
                        Color::Green
                    })
                    .bold(),
            ),
        ]),
    ];

    if let Some((msg, created, color)) = &state.status_msg
        && created.elapsed() < Duration::from_secs(4)
    {
        stat_lines.push(Line::from(vec![
            Span::styled("Action: ", Style::default().bold()),
            Span::styled(msg, Style::default().fg(*color).bold()),
        ]));
    }

    let stats_block = Block::default()
        .title(" Summary ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    f.render_widget(Paragraph::new(stat_lines).block(stats_block), top_chunks[1]);

    // 3. Active Accesses Table
    let table_block = Block::default()
        .title(" Active Captures & Evidence ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    if accesses.is_empty() {
        let empty_msg = Paragraph::new(Line::from(vec![
            Span::styled("✔ ", Style::default().fg(Color::Green).bold()),
            Span::styled(
                "No active capture detected. Microphone and camera are currently idle.",
                Style::default().fg(Color::Green),
            ),
        ]))
        .block(table_block);
        f.render_widget(empty_msg, chunks[2]);
    } else {
        let rows = accesses.iter().enumerate().map(|(idx, a)| {
            let res_cell = match a.resource {
                Resource::Microphone => Cell::from(state.lang.resource(a.resource))
                    .style(Style::default().fg(Color::Magenta).bold()),
                Resource::Camera => Cell::from(state.lang.resource(a.resource))
                    .style(Style::default().fg(Color::Cyan).bold()),
            };
            let act_str = state.lang.state_str(None, a.activity);
            let act_cell = match a.activity {
                Activity::Active => {
                    Cell::from(act_str).style(Style::default().fg(Color::Green).bold())
                }
                Activity::Ready => {
                    Cell::from(act_str).style(Style::default().fg(Color::Yellow).bold())
                }
            };
            let risk_color = match a.risk {
                Risk::Expected => Color::Green,
                Risk::Unexplained => Color::Yellow,
                Risk::Suspicious => Color::Rgb(255, 140, 0),
                Risk::Blocked => Color::Red,
            };
            let risk_cell = Cell::from(state.lang.risk_str(a.risk))
                .style(Style::default().fg(risk_color).bold());
            let app_cell =
                Cell::from(a.application.clone()).style(Style::default().fg(Color::White).bold());
            let pid_cell = Cell::from(a.pid.map_or("?".into(), |p| p.to_string()))
                .style(Style::default().fg(Color::Cyan));
            let parent_cell = Cell::from(a.parent_name.clone().unwrap_or_else(|| "-".into()))
                .style(Style::default().fg(Color::DarkGray));
            let sig_cell = match &a.signature {
                Some(s) if s.verified => {
                    Cell::from("Verified").style(Style::default().fg(Color::Green))
                }
                Some(_) => Cell::from("Unverified").style(Style::default().fg(Color::Red)),
                None => Cell::from("-").style(Style::default().fg(Color::DarkGray)),
            };

            let row = Row::new(vec![
                res_cell,
                act_cell,
                risk_cell,
                app_cell,
                pid_cell,
                parent_cell,
                sig_cell,
            ]);
            if idx == state.selected_access {
                row.style(Style::default().bg(Color::Rgb(30, 41, 59)))
            } else {
                row
            }
        });

        let header = Row::new(vec![
            Cell::from("Type").style(Style::default().bold()),
            Cell::from("State").style(Style::default().bold()),
            Cell::from("Risk").style(Style::default().bold()),
            Cell::from("Process").style(Style::default().bold()),
            Cell::from("PID").style(Style::default().bold()),
            Cell::from("Parent").style(Style::default().bold()),
            Cell::from("Signature").style(Style::default().bold()),
        ])
        .style(Style::default().fg(Color::Cyan));

        let widths = [
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Length(12),
            Constraint::Length(24),
            Constraint::Length(8),
            Constraint::Length(18),
            Constraint::Length(12),
        ];

        let table = Table::new(rows, widths).header(header).block(table_block);
        f.render_stateful_widget(table, chunks[2], &mut state.table_state);
    }

    // 4. Event Stream Log
    let log_block = Block::default()
        .title(" Event Stream ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let log_lines: Vec<Line> = state
        .events_log
        .iter()
        .rev()
        .take(5)
        .rev()
        .map(|(time, text, col)| {
            Line::from(vec![
                Span::styled(format!("{time}  "), Style::default().fg(Color::DarkGray)),
                Span::styled(text, Style::default().fg(*col)),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(log_lines).block(log_block), chunks[3]);

    // 5. Footer Shortcuts
    let footer_line = Line::from(vec![
        Span::styled(
            " [q] ",
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
        Span::raw("Quit  "),
        Span::styled(
            " [m] ",
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
        Span::raw("Mute/Unmute Mic  "),
        Span::styled(
            " [k] ",
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
        Span::raw("Terminate Process  "),
        Span::styled(
            " [↑/↓] ",
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
        Span::raw("Select  "),
        Span::styled(
            " [r] ",
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
        Span::raw("Refresh"),
    ]);
    f.render_widget(Paragraph::new(footer_line), chunks[4]);
}
