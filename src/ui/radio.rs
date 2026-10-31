use crate::app::{App, Cmd};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mopytui::radio::{Check, player::Command};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

/// Own radio controls before global Mopidy bindings (notably Space / s / >).
pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<Cmd> {
    let r = &mut app.radio;
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Cmd::Quit);
    }
    if r.collections.visible {
        if let Some(input) = &mut r.collections.input {
            match key.code {
                KeyCode::Esc => r.collections.input = None,
                KeyCode::Enter => r.save_collection(),
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) if !c.is_control() && input.chars().count() < 80 => input.push(c),
                _ => {}
            }
        } else if r.collections.update_confirm {
            match key.code {
                KeyCode::Char('y') => r.update_collection(),
                KeyCode::Esc | KeyCode::Char('n') => r.collections.update_confirm = false,
                _ => {}
            }
        } else if r.collections.delete_confirm {
            match key.code {
                KeyCode::Char('y') => r.delete_collection(),
                KeyCode::Esc | KeyCode::Char('n') => r.collections.delete_confirm = false,
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Esc | KeyCode::F(3) => r.collections.visible = false,
                KeyCode::Up | KeyCode::Char('k') => {
                    r.collections.cursor = r.collections.cursor.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    r.collections.cursor =
                        (r.collections.cursor + 1).min(r.collections.items.len().saturating_sub(1))
                }
                KeyCode::Enter => r.load_collection(),
                KeyCode::Char('s') => {
                    r.collections.input = Some(String::new());
                    r.collections.rename = false;
                }
                KeyCode::Char('r') => {
                    if let Some(c) = r.collections.items.get(r.collections.cursor) {
                        r.collections.input = Some(c.name.clone());
                        r.collections.rename = true;
                    }
                }
                KeyCode::Char('u') if !r.collections.items.is_empty() => {
                    r.collections.update_confirm = true
                }
                KeyCode::Delete | KeyCode::Char('d') if !r.collections.items.is_empty() => {
                    r.collections.delete_confirm = true
                }
                _ => {}
            }
        }
        return Some(Cmd::None);
    }
    if key.code == KeyCode::F(3) && r.settings.is_none() {
        r.open_collections();
        return Some(Cmd::None);
    }
    if let Some((draft, field)) = r.settings.as_mut() {
        match key.code {
            KeyCode::Esc => r.settings = None,
            KeyCode::Enter => {
                let cfg = draft.clone();
                if cfg.validate().is_ok() {
                    r.command(Command::Configure(cfg.clone()));
                    r.config = cfg.clone();
                    r.settings = None;
                    return Some(Cmd::SaveRadioNetwork(cfg));
                }
            }
            KeyCode::Tab | KeyCode::Down => *field = (*field + 1) % 7,
            KeyCode::BackTab | KeyCode::Up => *field = (*field + 6) % 7,
            KeyCode::Left | KeyCode::Right | KeyCode::Char('-' | '+' | '=') => {
                let increase = matches!(key.code, KeyCode::Right | KeyCode::Char('+' | '='));
                match *field {
                    0 => {
                        draft.rotation_min = if increase {
                            (draft.rotation_min + 1).min(10080)
                        } else {
                            draft.rotation_min.saturating_sub(1).max(1)
                        };
                        draft.rotation_max = draft.rotation_max.max(draft.rotation_min);
                    }
                    1 => {
                        draft.rotation_max = if increase {
                            (draft.rotation_max + 1).min(10080)
                        } else {
                            draft.rotation_max.saturating_sub(1).max(1)
                        };
                        draft.rotation_min = draft.rotation_min.min(draft.rotation_max);
                    }
                    4 => {
                        draft.preferred_network = match draft.preferred_network {
                            mopytui::radio::Network::Direct => mopytui::radio::Network::Proxy,
                            mopytui::radio::Network::Proxy => mopytui::radio::Network::Direct,
                        };
                    }
                    6 => { draft.sound_signals = !draft.sound_signals; }
                    5 => { draft.buffer_seconds = (draft.buffer_seconds + if increase { 1. } else { -1. }).clamp(0., 30.); }
                    _ => {
                        let value = if *field == 2 {
                            &mut draft.fade_out
                        } else {
                            &mut draft.fade_in
                        };
                        *value = ((*value * 10.).round() + if increase { 1. } else { -1. })
                            .clamp(1., 3000.)
                            / 10.;
                    }
                }
            }
            _ => {}
        }
        return Some(Cmd::None);
    }
    if key.code == KeyCode::F(2) {
        r.settings = Some((r.config.clone(), 0));
        return Some(Cmd::None);
    }
    if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
        let focus = if r.editing { 0 } else if r.search_view { 1 } else { 2 };
        let reverse = key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT);
        let next = (focus + if reverse { 2 } else { 1 }) % 3;
        r.editing = next == 0;
        r.search_view = next != 2;
        return Some(Cmd::None);
    }
    if r.editing {
        match key.code {
            KeyCode::Esc => {
                r.editing = false;
                r.search_view = false;
            }
            KeyCode::Enter => {
                r.editing = false;
                r.search_view = true;
                r.search();
            }
            KeyCode::Backspace => {
                r.query.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => r.query.push(c),
            _ => {}
        }
        return Some(Cmd::None);
    }
    match key.code {
        KeyCode::Char('/') | KeyCode::Char('a') => {
            r.search_view = true;
            r.editing = true;
        }
        KeyCode::Esc => {
            r.search_view = false;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if r.search_view {
                r.cursor = (r.cursor + 1).min(r.results.len().saturating_sub(1));
            } else {
                r.loop_cursor = (r.loop_cursor + 1).min(r.entries.len().saturating_sub(1));
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if r.search_view {
                r.cursor = r.cursor.saturating_sub(1);
            } else {
                r.loop_cursor = r.loop_cursor.saturating_sub(1);
            }
        }
        KeyCode::Char(' ') => {
            if r.search_view { r.choose_result(false); }
        }
        KeyCode::Enter => {
            if r.search_view { r.choose_result(true); }
            else if r.choose_loop() { return Some(Cmd::RadioStart); }
        }
        KeyCode::Char('n') | KeyCode::Char('>') => r.command(Command::Next),
        KeyCode::Char('<') => r.command(Command::Previous),
        KeyCode::Char('r') => r.command(Command::Restart),
        KeyCode::Char('p') => {
            if r.snapshot.running {
                r.command(Command::Pause);
            } else {
                return Some(Cmd::RadioStart);
            }
        }
        KeyCode::Char('s') => r.command(Command::Stop),
        KeyCode::Char('d') | KeyCode::Delete if !r.search_view => {
            if r.loop_cursor < r.entries.len() {
                r.entries.remove(r.loop_cursor);
            }
            r.loop_cursor = r.loop_cursor.min(r.entries.len().saturating_sub(1));
            r.sync();
        }
        KeyCode::Char('J') | KeyCode::Char('K') if !r.search_view => {
            let i = r.loop_cursor;
            let j = if key.code == KeyCode::Char('J') {
                (i + 1).min(r.entries.len().saturating_sub(1))
            } else {
                i.saturating_sub(1)
            };
            if i < r.entries.len() {
                r.entries.swap(i, j);
                r.loop_cursor = j;
                r.sync();
            }
        }
        // No radio key may accidentally seek/mute/stop Mopidy.
        KeyCode::Char('[' | ']' | 'm' | 'R' | 'T' | 'S' | 'C') => {}
        _ => return None,
    }
    Some(Cmd::None)
}

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let r = &app.radio;
    let chunks = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);
    let current_name = r
        .entries
        .iter()
        .find(|e| Some(e.station.id()) == r.snapshot.current.as_deref())
        .map(|e| e.station.name.as_str())
        .unwrap_or("—");
    let next_name = r
        .entries
        .iter()
        .find(|e| Some(e.station.id()) == r.snapshot.next.as_deref())
        .map(|e| e.station.name.as_str())
        .unwrap_or("—");
    let label_style = Style::default()
        .fg(app.theme.accent)
        .add_modifier(Modifier::BOLD);
    let label = |text: &'static str| Span::styled(text, label_style);
    let header = vec![
        Line::from(vec![
            label("Network: "),
            Span::raw(if r.snapshot.running { r.snapshot.network.clone() } else { r.config.preferred_description() }),
            label("   Buffer: "),
            Span::raw(format!("{:.0} s{}", r.config.buffer_seconds, r.snapshot.buffering.map(|p| format!(" · {p}%")).unwrap_or_default())),
        ]),
        Line::from(vec![
            label("Now: "),
            Span::styled(current_name.to_owned(), Style::default().fg(app.theme.accent_alt).add_modifier(Modifier::BOLD)),
            label("   Next: "),
            Span::styled(next_name.to_owned(), Style::default().fg(app.theme.accent_alt)),
        ]),
        Line::from(vec![
            label("Status: "),
            Span::raw(format!(
                "{}{}",
                if r.snapshot.paused { "PAUSED · " } else { "" },
                r.snapshot.status
            )),
            label("   Switch in: "),
            Span::raw(format!(
                "{:02}:{:02}",
                r.snapshot.remaining / 60,
                r.snapshot.remaining % 60
            )),
        ]),
        Line::from(vec![
            label("Range: "),
            Span::raw(format!(
                "{}–{} min",
                r.config.rotation_min, r.config.rotation_max
            )),
            label("   Fade out: "),
            Span::raw(format!("{:.1} s", r.config.fade_out)),
            label("   Fade in: "),
            Span::raw(format!("{:.1} s", r.config.fade_in)),
        ]),
    ];
    f.render_widget(
        Paragraph::new(header).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Radio Loop · independent audio ")
                .border_style(Style::default().fg(app.theme.border))
                .title_style(Style::default().fg(app.theme.accent_alt)),
        ),
        chunks[0],
    );
    if r.collections.visible {
        let rows: Vec<_> = r
            .collections
            .items
            .iter()
            .enumerate()
            .map(|(i, c)| {
                ListItem::new(format!(
                    "{} · {} stations · {}–{} min · {:.1}/{:.1} s",
                    c.name,
                    c.entries.len(),
                    c.rotation_min,
                    c.rotation_max,
                    c.fade_out,
                    c.fade_in
                ))
                .style(if i == r.collections.cursor {
                    label_style
                } else {
                    Style::default()
                })
            })
            .collect();
        let title = if let Some(input) = &r.collections.input {
            format!(
                " {}: {input}▏ ",
                if r.collections.rename {
                    "Rename"
                } else {
                    "Save current loop"
                }
            )
        } else if r.collections.update_confirm {
            " Replace saved collection with current loop? ".into()
        } else if r.collections.delete_confirm {
            " Delete collection? ".into()
        } else {
            " Radio Collections ".into()
        };
        f.render_widget(
            List::new(rows).block(Block::default().borders(Borders::ALL).title(title).border_style(Style::default().fg(app.theme.accent))),
            chunks[1],
        );
        f.render_widget(
            Paragraph::new(vec![
                Line::from(r.message.clone()),
                hint_line(
                    if r.collections.input.is_some() { &[("Enter", "save"), ("Esc", "cancel")] }
                    else if r.collections.update_confirm || r.collections.delete_confirm { &[("y", "confirm"), ("n/Esc", "cancel")] }
                    else { &[("Enter", "load"), ("s", "save as"), ("u", "update"), ("r", "rename"), ("d", "delete"), ("Esc", "back")] },
                    label_style,
                ),
            ]),
            chunks[2],
        );
        return;
    }
    if let Some((draft, field)) = &r.settings {
        let rows = [
            format!("Interval minimum: {} min", draft.rotation_min),
            format!("Interval maximum: {} min", draft.rotation_max),
            format!("Fade out: {:.1} s", draft.fade_out),
            format!("Fade in: {:.1} s", draft.fade_in),
            format!("Preferred network: {} · {}", if draft.preferred_network == mopytui::radio::Network::Proxy { "proxy" } else { "direct" }, if draft.effective_proxy().is_some() { "automatic fallback" } else { "proxy not configured; direct only" }),
            format!("Audio buffer: {:.0} s (0 = off)", draft.buffer_seconds),
            format!("Start / stop sounds: {}", if draft.sound_signals { "on" } else { "off" }),
        ];
        let items: Vec<_> = rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                ListItem::new(row).style(if i == *field {
                    label_style
                } else {
                    Style::default()
                })
            })
            .collect();
        let mut state = ListState::default().with_selected(Some(*field));
        f.render_stateful_widget(
            List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Radio settings · network defaults are saved ")
                    .border_style(Style::default().fg(app.theme.accent)),
            ),
            chunks[1],
            &mut state,
        );
        f.render_widget(
            Paragraph::new(vec![
                Line::from("Changing the interval starts a new timer when applied."),
                hint_line(
                    &[
                        ("↑/↓", "field"),
                        ("←/→ or +/-", "change"),
                        ("Enter", "apply"),
                        ("Esc", "cancel"),
                    ],
                    label_style,
                ),
            ]),
            chunks[2],
        );
        return;
    }
    let panes = Layout::vertical([Constraint::Length(3), Constraint::Percentage(50), Constraint::Percentage(50)]).split(chunks[1]);
    let border = |active: bool| Style::default().fg(if active { app.theme.accent } else { app.theme.border });
    f.render_widget(Paragraph::new(r.query.as_str()).block(Block::bordered()
        .title(panel_title("Search", &[("Enter", "search")], label_style)).border_style(border(r.editing))), panes[0]);
    if r.editing && panes[0].width > 2 && panes[0].height > 2 {
        use unicode_width::UnicodeWidthStr;
        f.set_cursor_position((panes[0].x + 1 + (r.query.width() as u16).min(panes[0].width.saturating_sub(3)), panes[0].y+1));
    }
    let items: Vec<ListItem> = r.results.iter().map(|row| {
        let (check, color) = match &row.check {
            Check::Unchecked => ("not checked".into(), app.theme.fg_muted),
            Check::Checking => ("checking…".into(), app.theme.warn),
            Check::Ok => ("OK".into(), app.theme.ok),
            Check::Dead(e) => (format!("DEAD: {e}"), app.theme.err),
        };
        let added = r.entries.iter().any(|e| e.station.id() == row.station.id());
        ListItem::new(Line::from(vec![Span::raw(format!("[{}] {}  {} {} kbps  ", if added { 'x' } else { ' ' }, row.station.name, row.station.codec, row.station.bitrate)), Span::styled(check, Style::default().fg(color).add_modifier(Modifier::BOLD))]))
    }).collect();
    let selected = Style::default().bg(app.theme.selection_bg).add_modifier(Modifier::BOLD);
    let mut results = ListState::default().with_offset(r.result_offset).with_selected(Some(r.cursor));
    f.render_stateful_widget(List::new(items).block(Block::bordered().title(panel_title("Results", &[("Space", "add"), ("Enter", "add + play")], label_style)).border_style(border(r.search_view && !r.editing))).highlight_style(selected).highlight_symbol("› "), panes[1], &mut results);
    let items: Vec<ListItem> = r.entries.iter().map(|entry| {
        let current = Some(entry.station.id()) == r.snapshot.current.as_deref();
        ListItem::new(Line::from(vec![
            Span::styled(if current { "▶ " } else { "  " }, Style::default().fg(app.theme.ok)),
            Span::styled(entry.station.name.clone(), Style::default().fg(if current { app.theme.accent_alt } else { app.theme.fg }).add_modifier(if current { Modifier::BOLD } else { Modifier::empty() })),
        ]))
    }).collect();
    let mut playing = ListState::default().with_offset(r.loop_offset).with_selected(Some(r.loop_cursor));
    f.render_stateful_widget(List::new(items).block(Block::bordered().title(panel_title("Playback", &[("Enter", "play station"), ("d", "remove"), ("J/K", "reorder")], label_style)).border_style(border(!r.search_view && !r.editing))).highlight_style(selected).highlight_symbol("› "), panes[2], &mut playing);
    let hints = hint_line(if r.editing {
        &[("Enter", "search"), ("Tab/Shift+Tab", "focus"), ("Esc", "playback"), ("F2", "settings"), ("F3", "collections")]
    } else {
        &[("Tab/Shift+Tab", "focus"), ("p", "pause/play"), ("s", "stop"), ("n/<", "next/prev"), ("+/-", "volume"), ("F2", "settings"), ("F3", "collections")]
    }, label_style);
    let footer = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(chunks[2]);
    let diagnostic = if r.searching {
        format!("{} · {} s", r.message, r.search_started.map(|t| t.elapsed().as_secs()).unwrap_or(0))
    } else if !r.search_view && !r.snapshot.last_error.is_empty() {
        format!("Last audio error: {}", r.snapshot.last_error)
    } else {
        r.message.clone()
    };
    f.render_widget(
        Paragraph::new(diagnostic).wrap(ratatui::widgets::Wrap { trim: true }),
        footer[0],
    );
    f.render_widget(Paragraph::new(hints), footer[1]);
    app.radio.result_offset = results.offset();
    app.radio.loop_offset = playing.offset();
    app.hit_regions.push((panes[0], crate::input::Hit::RadioQuery));
    for (area, offset, len, playback) in [(panes[1], results.offset(), app.radio.results.len(), false), (panes[2], playing.offset(), app.radio.entries.len(), true)] {
        for row in 0..area.height.saturating_sub(2) {
            let index = offset + row as usize;
            if index < len {
                app.hit_regions.push((Rect::new(area.x+1, area.y+1+row, area.width.saturating_sub(2), 1), if playback { crate::input::Hit::RadioPlaying(index) } else { crate::input::Hit::RadioResult(index) }));
            }
        }
    }
}

fn hint_line(items: &[(&str, &str)], style: Style) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, (key, action)) in items.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" · "));
        }
        spans.push(Span::styled((*key).to_owned(), style));
        spans.push(Span::raw(format!(" {action}")));
    }
    Line::from(spans)
}

fn panel_title(name: &str, items: &[(&str, &str)], style: Style) -> Line<'static> {
    let mut line = hint_line(items, style);
    line.spans.insert(0, Span::raw(format!(" {name} · ")));
    line
}
