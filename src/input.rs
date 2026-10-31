//! Pure key → Cmd dispatch. The async executor in `cmd.rs` turns Cmds into
//! JSON-RPC calls. Splitting it this way keeps key handling synchronous and
//! the render loop responsive.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Cmd, LibraryFocus, PlaylistsFocus, View};
use crate::mopidy::models::PlayState;

pub fn handle_key(app: &mut App, key: KeyEvent) -> Cmd {
    let text_input = app.view == View::Radio && ((app.radio.editing && !app.radio.collections.visible && app.radio.settings.is_none()) || app.radio.collections.input.is_some());
    let key = if text_input && !key.modifiers.contains(KeyModifiers::CONTROL) { key } else { normalize_key(key) };
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Left | KeyCode::Right) {
        let views = [View::Queue, View::Albums, View::Library, View::Playlists, View::NowPlaying, View::Radio, View::Info];
        let i = views.iter().position(|v| *v == app.view).unwrap_or(0);
        return switch_view(app, views[(i + if key.code == KeyCode::Right { 1 } else { 6 }) % 7]);
    }
    if app.view == View::Radio {
        if let Some(cmd) = crate::ui::radio::handle_key(app, key) { return cmd; }
    }

    if let Some(g) = global_key(app, key) {
        return g;
    }

    match app.view {
        View::Library => handle_library(app, key),
        View::Albums => handle_albums(app, key),
        View::Queue => handle_queue(app, key),
        View::Radio => Cmd::None,
        View::Playlists => handle_playlists(app, key),
        View::NowPlaying => handle_now_playing(app, key),
        View::Help | View::Info => handle_info(app, key),
    }
}

fn handle_albums(app: &mut App, key: KeyEvent) -> Cmd {
    use crate::app::AlbumsMode;
    match app.albums.mode {
        AlbumsMode::Grid => handle_albums_grid(app, key),
        AlbumsMode::Detail => handle_albums_detail(app, key),
    }
}

fn handle_albums_grid(app: &mut App, key: KeyEvent) -> Cmd {
    let len = app.albums.items.len();
    if len == 0 {
        if matches!(key.code, KeyCode::Char('r')) {
            return Cmd::LoadAlbums;
        }
        return Cmd::None;
    }
    // Cols is recomputed at render time; mirror the formula here so input
    // navigation jumps by the right amount.
    let cols = effective_cols(app);
    let cur = app.albums.grid_index;
    match key.code {
        KeyCode::Right | KeyCode::Char('l') => {
            if cur + 1 < len { app.albums.grid_index = cur + 1; }
            Cmd::None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if cur > 0 { app.albums.grid_index = cur - 1; }
            Cmd::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.albums.grid_index = (cur + cols).min(len - 1);
            Cmd::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if cur >= cols { app.albums.grid_index = cur - cols; }
            Cmd::None
        }
        KeyCode::PageDown => {
            app.albums.grid_index = (cur + cols * 3).min(len - 1);
            Cmd::None
        }
        KeyCode::PageUp => {
            app.albums.grid_index = cur.saturating_sub(cols * 3);
            Cmd::None
        }
        KeyCode::Home => { app.albums.grid_index = 0; Cmd::None }
        KeyCode::End => { app.albums.grid_index = len - 1; Cmd::None }
        KeyCode::Enter => {
            let uri = app.albums.items[cur].uri.clone();
            Cmd::OpenAlbumDetail(uri)
        }
        KeyCode::Char('a') => {
            let uri = app.albums.items[cur].uri.clone();
            Cmd::QueueAlbum(uri)
        }
        KeyCode::Char('p') => {
            let uri = app.albums.items[cur].uri.clone();
            Cmd::PlayAlbum(uri)
        }
        KeyCode::Char('f') => {
            let uri = app.albums.items[cur].uri.clone();
            Cmd::ToggleFavoriteAlbum(uri)
        }
        KeyCode::Char('r') => Cmd::LoadAlbums,
        _ => Cmd::None,
    }
}

/// Approximate grid columns count for keyboard navigation. Mirrors the
/// render-time formula so j/k jump rows that visually match the layout.
fn effective_cols(app: &App) -> usize {
    let _ = app;
    // Mirror render-time formula in ui/albums.rs (inner.width / 28). Without
    // plumbing terminal size here we default to 4 — matches typical
    // terminals at 120-cell width after chrome.
    4
}

fn handle_albums_detail(app: &mut App, key: KeyEvent) -> Cmd {
    let Some(detail) = app.albums.detail.as_mut() else { return Cmd::None };
    let len = detail.tracks.len();
    match key.code {
        KeyCode::Esc | KeyCode::Backspace => Cmd::BackToAlbumsGrid,
        KeyCode::Char('p') => Cmd::PlayAlbum(detail.card.uri.clone()),
        KeyCode::Char('a') => Cmd::QueueAlbum(detail.card.uri.clone()),
        KeyCode::Char('f') => Cmd::ToggleFavoriteAlbum(detail.card.uri.clone()),
        KeyCode::Down | KeyCode::Char('j') => {
            if len > 0 && detail.track_index + 1 < len { detail.track_index += 1; }
            Cmd::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if detail.track_index > 0 { detail.track_index -= 1; }
            Cmd::None
        }
        KeyCode::Enter => {
            if let Some(t) = detail.tracks.get(detail.track_index) {
                Cmd::Add(vec![t.uri.clone()])
            } else {
                Cmd::None
            }
        }
        _ => Cmd::None,
    }
}

fn global_key(app: &mut App, key: KeyEvent) -> Option<Cmd> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Some(Cmd::Quit),
        KeyCode::Esc if app.view == View::Help => {
            app.set_view(app.prev_view);
            Some(Cmd::None)
        }
        KeyCode::Char('?') => {
            if app.view == View::Help {
                app.set_view(app.prev_view);
            } else {
                app.set_view(View::Help);
            }
            Some(Cmd::None)
        }
        KeyCode::Char('1') => { app.set_view(View::Queue); Some(Cmd::RefreshQueue) }
        KeyCode::Char('2') => {
            app.set_view(View::Albums);
            // Lazy load on first visit.
            if !app.albums.loaded && !app.albums.loading {
                Some(Cmd::LoadAlbums)
            } else {
                Some(Cmd::None)
            }
        }
        KeyCode::Char('3') => { app.set_view(View::Library); Some(Cmd::None) }
        KeyCode::Char('4') => { app.set_view(View::Playlists); Some(Cmd::LoadPlaylists) }
        KeyCode::Char('5') => { app.set_view(View::NowPlaying); Some(Cmd::None) }
        KeyCode::Char('6') => { app.set_view(View::Radio); Some(Cmd::None) }
        KeyCode::Char('7') => { app.set_view(View::Info); Some(Cmd::None) }
        // Ordinary playback globals; Radio intercepts its controls first.
        KeyCode::Char(' ') => Some(Cmd::TogglePlayPause),
        KeyCode::Char('s') if !matches!(app.view, View::Library | View::Radio) => Some(Cmd::Stop),
        // `<` / `>` for prev/next — `p` stays free for "play" in
        // Albums grid and Albums detail. `n` is also
        // not globally bound so it doesn't shadow per-view actions.
        KeyCode::Char('>') => Some(Cmd::Next),
        KeyCode::Char('<') => Some(Cmd::Prev),
        KeyCode::Char('[') => Some(Cmd::SeekRelative(-10_000)),
        KeyCode::Char(']') => Some(Cmd::SeekRelative(10_000)),
        KeyCode::Left if key.modifiers.is_empty() && app.view == View::NowPlaying => {
            Some(Cmd::SeekRelative(-5_000))
        }
        KeyCode::Right if key.modifiers.is_empty() && app.view == View::NowPlaying => {
            Some(Cmd::SeekRelative(5_000))
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            let v = (app.active_volume() + 5).clamp(0, 100);
            Some(Cmd::Volume(v))
        }
        KeyCode::Char('-') => {
            let v = (app.active_volume() - 5).clamp(0, 100);
            Some(Cmd::Volume(v))
        }
        KeyCode::Char('m') if app.view != View::Radio => Some(Cmd::ToggleMute),
        KeyCode::Char('R') => Some(Cmd::ToggleRandom),
        KeyCode::Char('T') => Some(Cmd::ToggleRepeat),
        KeyCode::Char('S') => Some(Cmd::ToggleSingle),
        KeyCode::Char('C') => Some(Cmd::ToggleConsume),
        KeyCode::Char('L') => {
            app.show_lyrics = !app.show_lyrics;
            Some(Cmd::None)
        }
        KeyCode::Char('c') if !matches!(app.view, View::Radio) => {
            app.cover_fit_mode = match app.cover_fit_mode {
                crate::app::CoverFitMode::Fit => crate::app::CoverFitMode::Crop,
                crate::app::CoverFitMode::Crop => crate::app::CoverFitMode::Fit,
            };
            // Force protocol rebuild so the new resize strategy applies.
            app.cover_protocol = None;
            app.cover_protocol_uri = None;
            Some(Cmd::None)
        }
        KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
            Some(Cmd::RefreshAll)
        }
        _ => None,
    }
}

fn handle_library(app: &mut App, key: KeyEvent) -> Cmd {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => { select_delta(app, 1); Cmd::None }
        KeyCode::Up | KeyCode::Char('k') => { select_delta(app, -1); Cmd::None }
        KeyCode::PageDown => { select_delta(app, 10); Cmd::None }
        KeyCode::PageUp => { select_delta(app, -10); Cmd::None }
        KeyCode::Home => { select_to(app, 0); Cmd::None }
        KeyCode::End => {
            let len = entries_len(app);
            select_to(app, len.saturating_sub(1));
            Cmd::None
        }
        KeyCode::Tab | KeyCode::BackTab => {
            app.library.focus = match app.library.focus {
                LibraryFocus::Entries => LibraryFocus::Tracks,
                LibraryFocus::Tracks => LibraryFocus::Entries,
            };
            Cmd::None
        }
        KeyCode::Enter => library_open_selected(app),
        KeyCode::Backspace | KeyCode::Char('h') => Cmd::BrowseUp,
        KeyCode::Char('a') | KeyCode::Char('A') => library_add_selected(app, key.code == KeyCode::Char('A')),
        KeyCode::Char('f') => {
            // Favorite the selected album (Tidal goodies).
            if app.library.focus == LibraryFocus::Entries
                && let Some(i) = app.library.entries_state.selected()
                && let Some(e) = app.library.entries.get(i)
                && e.kind == "album"
            {
                return Cmd::ToggleFavoriteAlbum(e.uri.clone());
            }
            Cmd::None
        }
        KeyCode::Char('r') if key.modifiers.is_empty() => {
            let uri = app.library.entries
                .get(app.library.entries_state.selected().unwrap_or(0))
                .map(|e| e.uri.clone());
            Cmd::RefreshLibrary(uri)
        }
        KeyCode::Char('/') => {
            app.set_view(View::Radio);
            app.radio.editing = true;
            app.radio.search_view = true;
            Cmd::None
        }
        _ => Cmd::None,
    }
}

fn entries_len(app: &App) -> usize {
    match app.library.focus {
        LibraryFocus::Entries => app.library.entries.len(),
        LibraryFocus::Tracks => app.library.album_tracks.as_ref().map(|t| t.len()).unwrap_or(0),
    }
}

fn select_delta(app: &mut App, delta: i32) {
    let len = entries_len(app);
    if len == 0 { return; }
    let state = match app.library.focus {
        LibraryFocus::Entries => &mut app.library.entries_state,
        LibraryFocus::Tracks => &mut app.library.album_tracks_state,
    };
    let cur = state.selected().unwrap_or(0) as i32;
    let next = (cur + delta).clamp(0, len as i32 - 1) as usize;
    state.select(Some(next));
}

fn select_to(app: &mut App, idx: usize) {
    let state = match app.library.focus {
        LibraryFocus::Entries => &mut app.library.entries_state,
        LibraryFocus::Tracks => &mut app.library.album_tracks_state,
    };
    state.select(Some(idx));
}

fn library_open_selected(app: &mut App) -> Cmd {
    match app.library.focus {
        LibraryFocus::Entries => {
            let Some(idx) = app.library.entries_state.selected() else { return Cmd::None };
            let Some(e) = app.library.entries.get(idx).cloned() else { return Cmd::None };
            match e.kind.as_str() {
                "directory" => Cmd::BrowseInto(Some(e.uri), e.name),
                "album" => Cmd::OpenAlbum(e.uri),
                "track" => Cmd::Add(vec![e.uri]),
                "playlist" => Cmd::OpenPlaylist(e.uri),
                _ => Cmd::None,
            }
        }
        LibraryFocus::Tracks => {
            let Some(idx) = app.library.album_tracks_state.selected() else { return Cmd::None };
            if let Some(t) = app.library.album_tracks.as_ref().and_then(|v| v.get(idx)).cloned() {
                Cmd::Add(vec![t.uri])
            } else { Cmd::None }
        }
    }
}

fn library_add_selected(app: &App, also_play: bool) -> Cmd {
    let uri = match app.library.focus {
        LibraryFocus::Entries => app.library.entries
            .get(app.library.entries_state.selected().unwrap_or(0))
            .map(|e| e.uri.clone()),
        LibraryFocus::Tracks => app.library.album_tracks.as_ref()
            .and_then(|v| v.get(app.library.album_tracks_state.selected().unwrap_or(0)))
            .map(|t| t.uri.clone()),
    };
    let Some(u) = uri else { return Cmd::None };
    if also_play {
        // Adding then auto-play is handled in the Cmd executor.
        Cmd::Add(vec![u])
    } else {
        Cmd::Add(vec![u])
    }
}

fn handle_queue(app: &mut App, key: KeyEvent) -> Cmd {
    let len = app.queue.len();
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            let cur = app.queue_state.table.selected().unwrap_or(0) as i32;
            let next = (cur + 1).clamp(0, len.saturating_sub(1) as i32) as usize;
            app.queue_state.table.select(Some(next));
            Cmd::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let cur = app.queue_state.table.selected().unwrap_or(0) as i32;
            let next = (cur - 1).clamp(0, len.saturating_sub(1) as i32) as usize;
            app.queue_state.table.select(Some(next));
            Cmd::None
        }
        KeyCode::Enter => {
            let Some(i) = app.queue_state.table.selected() else { return Cmd::None };
            app.queue.get(i).map(|t| Cmd::Play(Some(t.tlid))).unwrap_or(Cmd::None)
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            let Some(i) = app.queue_state.table.selected() else { return Cmd::None };
            app.queue.get(i).map(|t| Cmd::RemoveTlid(t.tlid)).unwrap_or(Cmd::None)
        }
        KeyCode::Char('J') => {
            let Some(i) = app.queue_state.table.selected() else { return Cmd::None };
            if i + 1 >= len { return Cmd::None }
            Cmd::MoveQueue { start: i as u32, end: i as u32 + 1, to: i as u32 + 1 }
        }
        KeyCode::Char('K') => {
            let Some(i) = app.queue_state.table.selected() else { return Cmd::None };
            if i == 0 { return Cmd::None }
            Cmd::MoveQueue { start: i as u32, end: i as u32 + 1, to: i as u32 - 1 }
        }
        KeyCode::Char('X') => Cmd::ClearQueue,
        KeyCode::Char('Z') => Cmd::ShuffleQueue,
        _ => Cmd::None,
    }
}

fn handle_playlists(app: &mut App, key: KeyEvent) -> Cmd {
    let len_list = app.playlists.items.len();
    let len_tracks = app.playlists.current.as_ref().map(|p| p.tracks.len()).unwrap_or(0);
    match key.code {
        KeyCode::Tab | KeyCode::BackTab => {
            app.playlists.focus = match app.playlists.focus {
                PlaylistsFocus::List => PlaylistsFocus::Tracks,
                PlaylistsFocus::Tracks => PlaylistsFocus::List,
            };
            Cmd::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.playlists.focus {
                PlaylistsFocus::List => {
                    let cur = app.playlists.state.selected().unwrap_or(0) as i32;
                    let next = (cur + 1).clamp(0, len_list.saturating_sub(1) as i32) as usize;
                    app.playlists.state.select(Some(next));
                }
                PlaylistsFocus::Tracks => {
                    let cur = app.playlists.tracks_state.selected().unwrap_or(0) as i32;
                    let next = (cur + 1).clamp(0, len_tracks.saturating_sub(1) as i32) as usize;
                    app.playlists.tracks_state.select(Some(next));
                }
            }
            Cmd::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            match app.playlists.focus {
                PlaylistsFocus::List => {
                    let cur = app.playlists.state.selected().unwrap_or(0) as i32;
                    let next = (cur - 1).clamp(0, len_list.saturating_sub(1) as i32) as usize;
                    app.playlists.state.select(Some(next));
                }
                PlaylistsFocus::Tracks => {
                    let cur = app.playlists.tracks_state.selected().unwrap_or(0) as i32;
                    let next = (cur - 1).clamp(0, len_tracks.saturating_sub(1) as i32) as usize;
                    app.playlists.tracks_state.select(Some(next));
                }
            }
            Cmd::None
        }
        KeyCode::Enter => {
            match app.playlists.focus {
                PlaylistsFocus::List => {
                    let Some(i) = app.playlists.state.selected() else { return Cmd::None };
                    app.playlists.items.get(i).map(|p| Cmd::OpenPlaylist(p.uri.clone())).unwrap_or(Cmd::None)
                }
                PlaylistsFocus::Tracks => {
                    let Some(i) = app.playlists.tracks_state.selected() else { return Cmd::None };
                    let uri = app.playlists.current.as_ref().and_then(|p| p.tracks.get(i)).map(|t| t.uri.clone());
                    uri.map(|u| Cmd::Add(vec![u])).unwrap_or(Cmd::None)
                }
            }
        }
        KeyCode::Char('a') if app.playlists.focus == PlaylistsFocus::List => {
            // Add all tracks of current playlist to queue.
            let uris = app.playlists.current.as_ref().map(|p| {
                p.tracks.iter().map(|t| t.uri.clone()).collect::<Vec<_>>()
            }).unwrap_or_default();
            if uris.is_empty() { Cmd::None } else { Cmd::Add(uris) }
        }
        KeyCode::Char('D') => {
            // Delete selected playlist.
            let Some(i) = app.playlists.state.selected() else { return Cmd::None };
            app.playlists.items.get(i).map(|p| Cmd::DeletePlaylist(p.uri.clone())).unwrap_or(Cmd::None)
        }
        _ => Cmd::None,
    }
}

fn handle_now_playing(app: &App, key: KeyEvent) -> Cmd {
    match key.code {
        KeyCode::Char(' ') => Cmd::TogglePlayPause,
        KeyCode::Char('s') => Cmd::Stop,
        KeyCode::Char('f') => {
            // Favorite the album of the currently playing track.
            app.playback
                .current
                .as_ref()
                .and_then(|t| t.album.as_ref())
                .and_then(|a| a.uri.clone())
                .map(Cmd::ToggleFavoriteAlbum)
                .unwrap_or(Cmd::None)
        }
        _ => Cmd::None,
    }
}

fn handle_info(_app: &mut App, _key: KeyEvent) -> Cmd { Cmd::None }

// Silence unused-arg warnings for handlers that don't read app state today
// but might once features expand.
#[allow(dead_code)]
fn _suppress(_: PlayState) {}

#[cfg(test)]
mod radio_tests {
    use super::*;
    fn app() -> App {
        App::new(crate::config::AppConfig::default(), crate::mopidy::Client::new("127.0.0.1", 6680),
            std::sync::Arc::new(crate::images::ImageCache::new()), ratatui_image::picker::Picker::halfblocks(), None)
    }
    #[tokio::test]
    async fn navigation_keeps_seven_views_and_radio_owns_space() {
        let mut a = app(); a.set_view(View::Queue);
        for expected in [View::Albums, View::Library, View::Playlists, View::NowPlaying, View::Radio, View::Info, View::Queue] {
            handle_key(&mut a, KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL));
            assert_eq!(a.view, expected);
        }
        a.set_view(View::Radio);
        assert!(matches!(handle_key(&mut a, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)), Cmd::None));
        handle_key(&mut a, KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        handle_key(&mut a, KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert_eq!(a.radio.query, "q");
        assert!(!a.quit);
        handle_key(&mut a, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!a.radio.editing && !a.radio.search_view);
        a.radio.shutdown().await;
    }
    #[tokio::test]
    async fn header_uses_radio_metadata_and_state_instead_of_paused_mopidy() {
        let mut a = app();
        a.radio.snapshot.running = true;
        a.radio.snapshot.artist = "Radio Artist".into();
        a.radio.snapshot.title = "Radio Track".into();
        a.radio.snapshot.current_name = "Radio Station".into();
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(140, 30)).unwrap();
        terminal.draw(|frame| crate::ui::render(frame, &mut a)).unwrap();
        let buf=terminal.backend().buffer();
        let text: String=(0..6).flat_map(|y| (0..140).map(move |x| buf[(x,y)].symbol())).collect();
        for value in ["Radio Artist", "Radio Track", "Radio Station", "Radio playing"] { assert!(text.contains(value), "{text}"); }
        assert!(!text.contains("Nothing playing"));
        a.radio.shutdown().await;
    }
    #[tokio::test]
    async fn volume_follows_active_player_across_tabs() {
        let mut a = app(); a.playback.volume = 10;
        a.radio.snapshot.running = true; a.radio.snapshot.volume = 0.23;
        for view in [View::Radio, View::NowPlaying] {
            a.set_view(view);
            assert_eq!(a.active_volume(), 23);
            assert!(matches!(handle_key(&mut a, KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE)), Cmd::Volume(28)));
        }
        a.radio.snapshot.running = false;
        assert_eq!(a.active_volume(), 10);
        a.radio.shutdown().await;
    }
    #[tokio::test]
    async fn radio_settings_apply_and_cancel_without_playback_keys() {
        let mut a = app(); a.set_view(View::Radio);
        for code in [KeyCode::F(2), KeyCode::Right, KeyCode::Enter] {
            handle_key(&mut a, KeyEvent::new(code, KeyModifiers::NONE));
        }
        assert_eq!(a.radio.config.rotation_min, 24);
        assert!(a.radio.settings.is_none());
        for code in [KeyCode::F(2), KeyCode::Right, KeyCode::Esc] {
            handle_key(&mut a, KeyEvent::new(code, KeyModifiers::NONE));
        }
        assert_eq!(a.radio.config.rotation_min, 24);
        for code in [KeyCode::F(2), KeyCode::Down, KeyCode::Down, KeyCode::Right, KeyCode::Enter] {
            handle_key(&mut a, KeyEvent::new(code, KeyModifiers::NONE));
        }
        assert_eq!(a.radio.config.fade_out, 1.6);
        a.radio.shutdown().await;
    }
    #[tokio::test]
    async fn radio_focus_russian_text_and_mouse_are_consistent() {
        use mopytui::radio::{Check, ResultRow, search::Station};
        let mut a = app(); a.set_view(View::Radio);
        handle_key(&mut a, KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        for c in "джаз".chars() { handle_key(&mut a, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)); }
        assert_eq!(a.radio.query, "джаз");
        handle_key(&mut a, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(!a.radio.editing && a.radio.search_view);
        handle_key(&mut a, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(!a.radio.editing && !a.radio.search_view);
        handle_key(&mut a, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(a.radio.editing);
        handle_key(&mut a, KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert!(!a.radio.editing && !a.radio.search_view);
        for i in 0..30 { a.radio.results.push(ResultRow { station: Station { stationuuid: i.to_string(), name: format!("Station {i}"), ..Default::default() }, selected: false, check: Check::Ok }); }
        a.radio.cursor = 29;
        a.radio.choose_result(false);
        assert_eq!(a.radio.entries.len(), 1);
        let enabled = a.radio.entries[0].enabled;
        handle_key(&mut a, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert_eq!(a.radio.entries[0].enabled, enabled);
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 35)).unwrap();
        terminal.draw(|f| crate::ui::render(f, &mut a)).unwrap();
        let result = a.hit_regions.iter().find(|(_, h)| matches!(h, Hit::RadioResult(29))).unwrap().0;
        let playing = a.hit_regions.iter().find(|(_, h)| matches!(h, Hit::RadioPlaying(0))).unwrap().0;
        assert!(result.y < playing.y);
        let mouse = |r: ratatui::layout::Rect| crossterm::event::MouseEvent { kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left), column:r.x, row:r.y, modifiers:KeyModifiers::NONE };
        assert!(matches!(handle_mouse(&mut a, mouse(result)), Cmd::None));
        assert_eq!(a.radio.cursor, 29);
        assert!(matches!(handle_mouse(&mut a, mouse(playing)), Cmd::RadioStart));
        assert_eq!(a.radio.play_target.as_deref(), Some("29"));
        handle_key(&mut a, KeyEvent::new(KeyCode::Char('в'), KeyModifiers::NONE));
        assert!(a.radio.entries.is_empty()); // physical D removes, Russian text wasn't transliterated earlier
        let tab = a.hit_regions.iter().find(|(_, h)| matches!(h, Hit::Tab(View::Queue))).unwrap().0;
        assert!(matches!(handle_mouse(&mut a, mouse(tab)), Cmd::RefreshQueue));
        assert_eq!(a.view, View::Queue);
        a.radio.shutdown().await;
    }
    #[test]
    fn russian_shortcuts_preserve_case_and_modifiers() {
        for (ru,en) in [('й','q'), ('о','j'), ('О','J'), ('л','k'), ('з','p'), ('х','['), ('Ю','>'), ('Б','<')] {
            assert_eq!(normalize_key(KeyEvent::new(KeyCode::Char(ru), KeyModifiers::NONE)).code, KeyCode::Char(en));
        }
        let key = normalize_key(KeyEvent::new(KeyCode::Char('к'), KeyModifiers::CONTROL));
        assert_eq!(key.code, KeyCode::Char('r')); assert_eq!(key.modifiers, KeyModifiers::CONTROL);
    }
    #[tokio::test]
    async fn radio_render_has_unnumbered_tabs() {
        let mut a = app(); a.set_view(View::Radio);
        let backend = ratatui::backend::TestBackend::new(120, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::render(frame, &mut a)).unwrap();
        let buf = terminal.backend().buffer();
        let tabs: String = (0..120).map(|x| buf[(x, 5)].symbol()).collect();
        for name in ["Queue", "Albums", "Library", "Playlists", "Playing", "Radio", "Info"] { assert!(tabs.contains(name), "{tabs}"); }
        assert!(!tabs.contains("Search") && !tabs.contains("Stats"));
        assert!(!tabs.chars().any(|c| c.is_ascii_digit()));
        a.radio.shutdown().await;
    }
}

#[derive(Clone, Copy)]
pub enum Hit {
    Tab(View), RadioQuery, RadioResult(usize), RadioPlaying(usize),
    Queue(usize), LibraryEntry(usize), LibraryTrack(usize), Playlist(usize), PlaylistTrack(usize), Album(usize), AlbumTrack(usize), Body,
}
pub fn switch_view(app: &mut App, view: View) -> Cmd {
    app.set_view(view);
    match view {
        View::Albums if !app.albums.loaded && !app.albums.loading => Cmd::LoadAlbums,
        View::Queue => Cmd::RefreshQueue,
        View::Playlists if app.playlists.items.is_empty() => Cmd::LoadPlaylists,
        _ => Cmd::None,
    }
}
fn normalize_key(mut key: KeyEvent) -> KeyEvent {
    if let KeyCode::Char(c) = key.code {
        let ru = "йцукенгшщзхъфывапролджэячсмитьбюёЙЦУКЕНГШЩЗХЪФЫВАПРОЛДЖЭЯЧСМИТЬБЮЁ";
        let en = "qwertyuiop[]asdfghjkl;'zxcvbnm,.`QWERTYUIOP{}ASDFGHJKL:\"ZXCVBNM<>~";
        if let Some(i) = ru.chars().position(|r| r == c) { key.code = KeyCode::Char(en.chars().nth(i).unwrap()); }
        // Russian layout uses Shift+7 for '?'; Shift+. for the physical slash key.
        if c == ',' && key.modifiers.contains(KeyModifiers::SHIFT) { key.code = KeyCode::Char('?'); }
        if c == '.' && key.modifiers.is_empty() { key.code = KeyCode::Char('/'); }
    }
    key
}
pub fn handle_mouse(app: &mut App, mouse: crossterm::event::MouseEvent) -> Cmd {
    use crossterm::event::{MouseEventKind, MouseButton};
    let point = ratatui::layout::Position::new(mouse.column, mouse.row);
    let Some(hit) = app.hit_regions.iter().rev().find(|(r, _)| r.contains(point)).map(|(_, h)| *h) else { return Cmd::None; };
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) { return Cmd::None; }
    match hit {
        Hit::Tab(view) => return switch_view(app, view),
        Hit::RadioQuery => { app.radio.editing = true; app.radio.search_view = true; return Cmd::None; }
        Hit::RadioResult(i) => { app.radio.cursor = i; app.radio.editing = false; app.radio.search_view = true; app.radio.choose_result(false); return Cmd::None; }
        Hit::RadioPlaying(i) => { app.radio.loop_cursor = i; app.radio.editing = false; app.radio.search_view = false; return if app.radio.choose_loop() { Cmd::RadioStart } else { Cmd::None }; }
        Hit::Queue(i) => app.queue_state.table.select(Some(i)),
        Hit::LibraryEntry(i) => { app.library.focus = LibraryFocus::Entries; app.library.entries_state.select(Some(i)); }
        Hit::LibraryTrack(i) => { app.library.focus = LibraryFocus::Tracks; app.library.album_tracks_state.select(Some(i)); }
        Hit::Playlist(i) => { app.playlists.focus = PlaylistsFocus::List; app.playlists.state.select(Some(i)); }
        Hit::PlaylistTrack(i) => { app.playlists.focus = PlaylistsFocus::Tracks; app.playlists.tracks_state.select(Some(i)); }
        Hit::Album(i) => app.albums.grid_index = i,
        Hit::AlbumTrack(i) => { if let Some(d) = &mut app.albums.detail { d.track_index = i; } }
        Hit::Body => {},
    }
    handle_key(app, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE))
}
