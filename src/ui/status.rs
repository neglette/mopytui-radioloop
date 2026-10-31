use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

use crate::app::{App, StatusKind};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    // If a flash status is set, it takes over the whole row.
    if !app.status.message.is_empty() {
        let color = match app.status.kind {
            StatusKind::Ok => theme.ok,
            StatusKind::Warn => theme.warn,
            StatusKind::Err => theme.err,
            StatusKind::Info => theme.fg_muted,
        };
        f.render_widget(
            Paragraph::new(Span::styled(
                format!(" {} ", app.status.message),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            area,
        );
        return;
    }

    use crate::app::View;
    let mut hints = vec![("Ctrl+←/→", "tabs")];
    match app.view {
        View::Radio => hints.extend([("Tab/Shift+Tab", "focus"), ("F2", "settings"), ("F3", "collections")]),
        View::Queue => hints.extend([("Enter", "play"), ("Space", "pause/play"), ("d", "remove"), ("J/K", "reorder")]),
        View::Albums => hints.extend([("Enter", "open/add"), ("p", "play album"), ("a", "queue"), ("f", "favorite"), ("Esc", "back")]),
        View::Library => hints.extend([("Tab", "focus"), ("Enter", "open/add"), ("a", "queue"), ("Backspace", "up")]),
        View::Playlists => hints.extend([("Tab", "focus"), ("Enter", "open/add"), ("a", "queue playlist"), ("D", "delete")]),
        _ => hints.extend([("Space", "pause/play"), ("s", "stop"), ("</>", "prev/next")]),
    }
    if app.view != View::Radio { hints.extend([("+/-", "volume"), ("?", "help")]); }
    f.render_widget(Paragraph::new(super::keys(&hints, app)), area);

}
