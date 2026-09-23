use crate::{
    frontends::cli::Filter,
    i18n::Language,
    model::{Access, Activity, CollectorState, Device, MicrophoneMuteState, Resource, Risk},
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

struct TerminalSessionGuard;

impl Drop for TerminalSessionGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

fn text(lang: Language, values: [&'static str; 7]) -> &'static str {
    values[match lang {
        Language::En => 0,
        Language::Fr => 1,
        Language::De => 2,
        Language::Es => 3,
        Language::Ja => 4,
        Language::Zh => 5,
        Language::Ru => 6,
    }]
}

pub fn run_tui(monitor: PlatformMonitor, lang: Language) -> Result<()> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let _session = TerminalSessionGuard;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to initialize ratatui terminal")?;

    let result = tui_loop(&mut terminal, monitor, lang);
    let _ = terminal.show_cursor();
    result
}

struct TuiState {
    monitor: PlatformMonitor,
    lang: Language,
    selected_access: usize,
    table_state: TableState,
    events_log: Vec<(String, String, Color)>,
    status_msg: Option<(String, Instant, Color)>,
    mute_state: MicrophoneMuteState,
    devices: Vec<Device>,
    health_error: Option<String>,
    pending_kill: Option<(String, u32, Instant)>,
}

fn tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    monitor: PlatformMonitor,
    lang: Language,
) -> Result<()> {
    let devices = monitor.devices().unwrap_or_default();
    let mute_state = monitor
        .microphone_mute_state()
        .unwrap_or(MicrophoneMuteState::Unavailable);

    let mut state = TuiState {
        monitor,
        lang,
        selected_access: 0,
        table_state: TableState::default(),
        events_log: Vec::new(),
        status_msg: None,
        mute_state,
        devices,
        health_error: None,
        pending_kill: None,
    };

    let filter = Filter {
        include_ready: true,
        ..Filter::default()
    };
    let mut last_poll = Instant::now() - Duration::from_secs(1);
    let mut current_accesses: Vec<Access> = Vec::new();

    loop {
        // Poll state every 250ms
        if last_poll.elapsed() >= Duration::from_millis(250) {
            last_poll = Instant::now();
            match state.monitor.snapshot((&filter).into()) {
                Ok(snapshot) => {
                    for access in &snapshot.accesses {
                        if !current_accesses.iter().any(|a| a.key == access.key) {
                            let time = Local::now().format("%H:%M:%S").to_string();
                            let text =
                                format!("START {} ({})", access.application, access.resource);
                            state.events_log.push((time, text, Color::Green));
                            if state.events_log.len() > 100 {
                                state.events_log.remove(0);
                            }
                        }
                    }
                    for previous in &current_accesses {
                        if !snapshot.accesses.iter().any(|a| a.key == previous.key) {
                            let time = Local::now().format("%H:%M:%S").to_string();
                            let text =
                                format!("STOP  {} ({})", previous.application, previous.resource);
                            state.events_log.push((time, text, Color::DarkGray));
                            if state.events_log.len() > 100 {
                                state.events_log.remove(0);
                            }
                        }
                    }
                    state.health_error = snapshot
                        .collectors
                        .iter()
                        .find(|collector| collector.state != CollectorState::Healthy)
                        .map(|collector| {
                            format!(
                                "{}: {}",
                                collector.collector,
                                collector.detail.as_deref().unwrap_or("degraded")
                            )
                        });
                    current_accesses = snapshot.accesses;
                    state.selected_access = state
                        .selected_access
                        .min(current_accesses.len().saturating_sub(1));
                }
                Err(error) => state.health_error = Some(format!("collection failed: {error:#}")),
            }
            state.mute_state = state
                .monitor
                .microphone_mute_state()
                .unwrap_or(MicrophoneMuteState::Unavailable);
        }

        // Draw UI
        terminal.draw(|f| draw_ui(f, &mut state, &current_accesses))?;

        // Handle inputs with 50ms timeout
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Esc if state.pending_kill.take().is_some() => {
                    state.status_msg = Some((
                        "Termination cancelled".to_owned(),
                        Instant::now(),
                        Color::Yellow,
                    ));
                }
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('m') => match state.monitor.toggle_microphone_mute() {
                    Ok(new_mute) => {
                        state.mute_state = if new_mute {
                            MicrophoneMuteState::Muted
                        } else {
                            MicrophoneMuteState::Unmuted
                        };
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
                    Err(error) => {
                        state.status_msg = Some((
                            format!("Mute action failed: {error:#}"),
                            Instant::now(),
                            Color::Yellow,
                        ));
                    }
                },
                KeyCode::Char('k') => {
                    if let Some(target) = current_accesses.get(state.selected_access)
                        && let Some(pid) = target.pid
                    {
                        let confirmed = state.pending_kill.as_ref().is_some_and(
                            |(key, pending_pid, created)| {
                                key == &target.key
                                    && *pending_pid == pid
                                    && created.elapsed() < Duration::from_secs(3)
                            },
                        );
                        if confirmed {
                            state.pending_kill = None;
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
                                Err(error) => {
                                    state.status_msg = Some((
                                        format!("Failed to terminate PID {pid}: {error:#}"),
                                        Instant::now(),
                                        Color::Yellow,
                                    ));
                                }
                            }
                        } else {
                            state.pending_kill = Some((target.key.clone(), pid, Instant::now()));
                            state.status_msg = Some((
                                format!(
                                    "Press [k] again within 3s to terminate {} (PID {})",
                                    target.application, pid
                                ),
                                Instant::now(),
                                Color::Yellow,
                            ));
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
    let mute_badge = match state.mute_state {
        MicrophoneMuteState::Muted => Span::styled(
            " [MIC MUTED] ",
            Style::default().bg(Color::Red).fg(Color::White).bold(),
        ),
        MicrophoneMuteState::Unmuted => Span::styled(
            " [MIC ON] ",
            Style::default().bg(Color::Green).fg(Color::Black).bold(),
        ),
        MicrophoneMuteState::Mixed => Span::styled(
            " [MIC MIXED] ",
            Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
        ),
        MicrophoneMuteState::Unavailable => Span::styled(
            " [NO MIC] ",
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
    };

    let title_line = Line::from(vec![
        Span::styled(" miccamwatch ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(
            format!("v{} ", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
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
            text(
                state.lang,
                [
                    "No capture devices found",
                    "Aucun périphérique de capture",
                    "Keine Aufnahmegeräte gefunden",
                    "No se encontraron dispositivos",
                    "キャプチャデバイスなし",
                    "未找到采集设备",
                    "Устройства захвата не найдены",
                ],
            ),
            Style::default().fg(Color::DarkGray),
        )));
    }
    let dev_block = Block::default()
        .title(text(
            state.lang,
            [
                " Devices ",
                " Périphériques ",
                " Geräte ",
                " Dispositivos ",
                " デバイス ",
                " 设备 ",
                " Устройства ",
            ],
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    f.render_widget(Paragraph::new(dev_lines).block(dev_block), top_chunks[0]);

    // Stats & Status Message
    let active_mics = accesses
        .iter()
        .filter(|a| a.resource == Resource::Microphone && a.activity == Activity::Active)
        .count();
    let active_cams = accesses
        .iter()
        .filter(|a| a.resource == Resource::Camera && a.activity == Activity::Active)
        .count();
    let ready_cams = accesses
        .iter()
        .filter(|a| a.resource == Resource::Camera && a.activity == Activity::Ready)
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
            Span::raw("Camera-ready pipelines: "),
            Span::styled(
                ready_cams.to_string(),
                Style::default().fg(Color::Yellow).bold(),
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
    if let Some(error) = &state.health_error {
        stat_lines.push(Line::from(vec![
            Span::styled("Telemetry: ", Style::default().bold()),
            Span::styled(error, Style::default().fg(Color::Red).bold()),
        ]));
    }

    let stats_block = Block::default()
        .title(text(
            state.lang,
            [
                " Summary ",
                " Résumé ",
                " Übersicht ",
                " Resumen ",
                " 概要 ",
                " 摘要 ",
                " Сводка ",
            ],
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    f.render_widget(Paragraph::new(stat_lines).block(stats_block), top_chunks[1]);

    // 3. Active Accesses Table
    let table_block = Block::default()
        .title(text(
            state.lang,
            [
                " Observed Access & Camera-Ready Pipelines ",
                " Accès observés et pipelines caméra prêts ",
                " Beobachtete Zugriffe und Kamerabereitschaft ",
                " Accesos observados y cámaras preparadas ",
                " 監視中のアクセスとカメラ準備状態 ",
                " 已观察访问和摄像头就绪管线 ",
                " Наблюдаемые доступы и готовность камеры ",
            ],
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    if accesses.is_empty() {
        let empty_msg = Paragraph::new(Line::from(vec![
            Span::styled("✔ ", Style::default().fg(Color::Green).bold()),
            Span::styled(
                text(
                    state.lang,
                    [
                        "No active capture detected. Microphone and camera are idle.",
                        "Aucune capture active. Le microphone et la caméra sont inactifs.",
                        "Keine aktive Aufnahme. Mikrofon und Kamera sind inaktiv.",
                        "Sin captura activa. Micrófono y cámara inactivos.",
                        "アクティブなキャプチャはありません。",
                        "未检测到活动采集。",
                        "Активный захват не обнаружен.",
                    ],
                ),
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
        .title(text(
            state.lang,
            [
                " Event Stream ",
                " Flux d’événements ",
                " Ereignisse ",
                " Eventos ",
                " イベント ",
                " 事件流 ",
                " События ",
            ],
        ))
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
        Span::raw(text(
            state.lang,
            [
                "Quit  ",
                "Quitter  ",
                "Beenden  ",
                "Salir  ",
                "終了  ",
                "退出  ",
                "Выход  ",
            ],
        )),
        Span::styled(
            " [m] ",
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
        Span::raw(text(
            state.lang,
            [
                "Mute/Unmute Mic  ",
                "Couper/activer micro  ",
                "Mikrofon umschalten  ",
                "Silenciar/activar micro  ",
                "マイク切替  ",
                "麦克风静音切换  ",
                "Микрофон вкл/выкл  ",
            ],
        )),
        Span::styled(
            " [k] ",
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
        Span::raw(text(
            state.lang,
            [
                "Terminate Process  ",
                "Terminer le processus  ",
                "Prozess beenden  ",
                "Terminar proceso  ",
                "プロセス終了  ",
                "终止进程  ",
                "Завершить процесс  ",
            ],
        )),
        Span::styled(
            " [↑/↓] ",
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
        Span::raw(text(
            state.lang,
            [
                "Select  ",
                "Sélectionner  ",
                "Auswählen  ",
                "Seleccionar  ",
                "選択  ",
                "选择  ",
                "Выбрать  ",
            ],
        )),
        Span::styled(
            " [r] ",
            Style::default().bg(Color::DarkGray).fg(Color::White).bold(),
        ),
        Span::raw(text(
            state.lang,
            [
                "Refresh",
                "Actualiser",
                "Aktualisieren",
                "Actualizar",
                "更新",
                "刷新",
                "Обновить",
            ],
        )),
    ]);
    f.render_widget(Paragraph::new(footer_line), chunks[4]);
}
