//! Radio owns its decks and clock; no Mopidy queue/playback calls live here.
//! Deck is the pipeline boundary. Future crossfade can retain outgoing and
//! incoming Decks simultaneously, applying complementary gains in this actor.
use super::{Config, Entry, probe};
use anyhow::Result;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin},
    sync::mpsc,
};

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub running: bool,
    pub paused: bool,
    pub current: Option<String>,
    pub next: Option<String>,
    pub remaining: u64,
    pub volume: f64,
    pub status: String,
    pub last_error: String,
    pub current_name: String,
    pub current_url: String,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub station_index: usize,
    pub station_count: usize,
    pub elapsed: u64,
    pub network: String,
    pub buffering: Option<u8>,
}
#[derive(Debug)]
pub enum Command {
    Replace(Vec<Entry>),
    Configure(Config),
    Start,
    Select(String),
    StopNow,
    Next,
    Previous,
    SetPaused(bool),
    SetVolume(f64),
    Restart,
    Pause,
    Stop,
    Volume(f64),
    Shutdown,
    Remote(Box<Command>, tokio::sync::oneshot::Sender<()>),
}
pub struct Handle {
    tx: mpsc::UnboundedSender<Command>,
    state: Arc<Mutex<Snapshot>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Handle {
    pub fn new(cfg: Config) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let state = Arc::new(Mutex::new(Snapshot::default()));
        let task = tokio::spawn(run(cfg, rx, state.clone()));
        Self {
            tx,
            state,
            task: Some(task),
        }
    }
    pub fn control(&self) -> Control {
        Control {
            tx: self.tx.clone(),
            state: self.state.clone(),
        }
    }
    pub fn send(&self, command: Command) {
        let _ = self.tx.send(command);
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().clone()
    }
    pub async fn shutdown(&mut self) {
        self.send(Command::Shutdown);
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}
#[derive(Clone)]
pub struct Control {
    tx: mpsc::UnboundedSender<Command>,
    state: Arc<Mutex<Snapshot>>,
}
impl Control {
    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().clone()
    }
    pub fn send(&self, command: Command) {
        let _ = self.tx.send(command);
    }
    pub async fn send_confirmed(&self, command: Command) -> anyhow::Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(Command::Remote(Box::new(command), tx))
            .map_err(|_| anyhow::anyhow!("radio player stopped"))?;
        rx.await?;
        Ok(())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.send(Command::Shutdown);
    }
}

#[derive(Debug)]
enum DeckEvent {
    Audio,
    Buffering(u8),
    Metadata(String, String, String),
    Error(String),
}
/// Drain both pipes and report a terminal error once. EOF must never replace
/// the structured GStreamer error that immediately preceded it.
async fn read_events(
    output: impl AsyncRead + Unpin,
    diagnostics: impl AsyncRead + Unpin,
    tx: mpsc::UnboundedSender<DeckEvent>,
) {
    let mut output = BufReader::new(output).lines();
    let mut diagnostics = BufReader::new(diagnostics).lines();
    let (mut stdout_open, mut stderr_open) = (true, true);
    let mut reported = false;
    let mut tail = std::collections::VecDeque::new();
    while stdout_open || stderr_open {
        tokio::select! {
            line = output.next_line(), if stdout_open => {
                match line {
                    Ok(Some(line)) => {
                        match serde_json::from_str::<serde_json::Value>(&line) {
                            Ok(value) if value["event"] == "audio" && !reported => {
                                if tx.send(DeckEvent::Audio).is_err() { return; }
                            }
                            Ok(value) if value["event"] == "buffering" && !reported => {
                                let percent = value["percent"].as_u64().unwrap_or(0).min(100) as u8;
                                if tx.send(DeckEvent::Buffering(percent)).is_err() { return; }
                            }
                            Ok(value) if value["event"] == "metadata" && !reported => {
                                let field = |key: &str| value[key].as_str().unwrap_or("").chars().filter(|c| !c.is_control()).take(512).collect::<String>();
                                if tx.send(DeckEvent::Metadata(field("artist"), field("title"), field("album"))).is_err() { return; }
                            }
                            Ok(value) if value["event"] == "error" && !reported => {
                                reported = true;
                                let message = value["message"].as_str().unwrap_or("GStreamer error").to_owned();
                                tracing::warn!(error = %message, "radio backend error");
                                if tx.send(DeckEvent::Error(message)).is_err() { return; }
                            }
                            _ => {}
                        }
                    }
                    Ok(None) => stdout_open = false,
                    Err(error) => {
                        stdout_open = false;
                        tail.push_back(format!("reading audio backend output: {error}"));
                    }
                }
            }
            line = diagnostics.next_line(), if stderr_open => {
                match line {
                    Ok(Some(line)) => {
                        let line: String = line.chars().take(1024).collect();
                        tracing::warn!(detail = %line, "radio backend stderr");
                        tail.push_back(line);
                        if tail.len() > 6 { tail.pop_front(); }
                    }
                    _ => stderr_open = false,
                }
            }
        }
    }
    if !reported {
        let reason = if tail.is_empty() {
            "without an error message".to_owned()
        } else {
            tail.into_iter().collect::<Vec<_>>().join(" | ")
        };
        let _ = tx.send(DeckEvent::Error(format!("Audio backend exited: {reason}")));
    }
}

struct Deck {
    child: Child,
    input: ChildStdin,
    events: mpsc::UnboundedReceiver<DeckEvent>,
    reader: tokio::task::JoinHandle<()>,
}
impl Deck {
    fn open(python: &str, url: &str, proxy: Option<&str>, buffer_seconds: f64) -> Result<Self> {
        let (mut child, input, output) = probe::spawn_buffered(python, url, "play", proxy, buffer_seconds)?;
        let diagnostics = child.stderr.take().expect("piped backend stderr");
        let (tx, events) = mpsc::unbounded_channel();
        let reader = tokio::spawn(read_events(output, diagnostics, tx));
        Ok(Self {
            child,
            input,
            events,
            reader,
        })
    }
    async fn control(&mut self, value: serde_json::Value) -> Result<()> {
        let line = format!("{value}\n");
        tokio::time::timeout(
            Duration::from_millis(200),
            self.input.write_all(line.as_bytes()),
        )
        .await??;
        Ok(())
    }
    async fn close(mut self) {
        let _ = self.control(serde_json::json!({"stop": true})).await;
        if tokio::time::timeout(Duration::from_millis(300), self.child.wait())
            .await
            .is_err()
        {
            let _ = self.child.kill().await;
            let _ = self.child.wait().await;
        }
        self.reader.abort();
    }
}
impl Drop for Deck {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Idle,
    Starting {
        retry: bool,
        until: f64,
    },
    FadeIn {
        since: f64,
    },
    Playing,
    FadeOut {
        since: f64,
        target: Option<String>,
        gain: f64,
    },
    RetryWait {
        until: f64,
    },
}
struct Engine {
    cfg: Config,
    entries: Vec<Entry>,
    current: Option<String>,
    phase: Phase,
    paused: bool,
    now: f64,
    deadline: f64,
    last_audio: f64,
    audio_started: f64,
    retry_active: bool,
    gain: f64,
    volume: f64,
    status: String,
    last_error: String,
    artist: String,
    title: String,
    album: String,
    route: usize,
    routes: Vec<Option<String>>,
    route_fresh: bool,
    buffering: Option<u8>,
    buffer_since: f64,
}
impl Engine {
    fn new(cfg: Config) -> Self {
        Self {
            cfg: cfg.clone(),
            entries: vec![],
            current: None,
            phase: Phase::Idle,
            paused: false,
            now: 0.,
            deadline: 0.,
            last_audio: 0.,
            audio_started: 0.,
            retry_active: false,
            gain: 0.,
            volume: 0.8,
            status: "Stopped".into(),
            last_error: String::new(),
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            route: 0,
            routes: cfg.routes(),
            route_fresh: false,
            buffering: None,
            buffer_since: 0.,
        }
    }
    fn first(&self) -> Option<String> {
        self.entries
            .iter()
            .find(|e| e.enabled)
            .map(|e| e.station.id().into())
    }
    fn next(&self) -> Option<String> {
        let len = self.entries.len();
        let start = self
            .entries
            .iter()
            .position(|e| Some(e.station.id()) == self.current.as_deref());
        (1..=len)
            .map(|offset| start.map_or(offset - 1, |i| (i + offset) % len))
            .find_map(|i| {
                self.entries[i]
                    .enabled
                    .then(|| self.entries[i].station.id().into())
            })
    }
    fn previous(&self) -> Option<String> {
        let len = self.entries.len();
        let start = self
            .entries
            .iter()
            .position(|e| Some(e.station.id()) == self.current.as_deref())
            .unwrap_or(0);
        (1..=len).find_map(|offset| {
            let i = (start + len - offset) % len;
            self.entries[i]
                .enabled
                .then(|| self.entries[i].station.id().into())
        })
    }
    fn transition(&mut self, target: Option<String>) {
        self.buffering = None;
        if self.paused {
            self.gain = 0.;
        }
        self.paused = false;
        self.phase = Phase::FadeOut {
            since: self.now,
            target,
            gain: self.gain,
        };
        self.status = "Fade out".into();
    }
    fn replace(&mut self, entries: Vec<Entry>) {
        let old_next = self.next();
        self.entries = entries;
        if self.phase == Phase::Idle {
            return;
        }
        let current_valid = self
            .entries
            .iter()
            .any(|e| e.enabled && Some(e.station.id()) == self.current.as_deref());
        if !current_valid {
            let next = old_next
                .filter(|id| {
                    self.entries
                        .iter()
                        .any(|e| e.enabled && e.station.id() == id)
                })
                .or_else(|| self.first());
            self.transition(next);
        } else if let Phase::FadeOut {
            target: Some(id), ..
        } = &self.phase
        {
            if !self
                .entries
                .iter()
                .any(|e| e.enabled && e.station.id() == id)
            {
                let next = self.next();
                if let Phase::FadeOut { target, .. } = &mut self.phase {
                    *target = next;
                }
            }
        }
    }
    fn snapshot(&self) -> Snapshot {
        let index = self
            .entries
            .iter()
            .position(|e| Some(e.station.id()) == self.current.as_deref());
        let entry = index.map(|i| &self.entries[i]);
        Snapshot {
            running: self.phase != Phase::Idle,
            paused: self.paused,
            current: self.current.clone(),
            next: self.next(),
            remaining: (self.deadline - self.now).max(0.).ceil() as u64,
            volume: self.volume,
            status: self.status.clone(),
            last_error: self.last_error.clone(),
            artist: self.artist.clone(),
            title: self.title.clone(),
            album: self.album.clone(),
            current_name: entry.map(|e| e.station.name.clone()).unwrap_or_default(),
            current_url: entry
                .map(|e| e.station.stream_url().to_owned())
                .unwrap_or_default(),
            station_index: index.unwrap_or(0),
            station_count: self.entries.len(),
            elapsed: (self.now - self.audio_started).max(0.) as u64,
            network: super::proxy_description(self.routes.get(self.route).and_then(|p| p.as_deref())),
            buffering: self.buffering,

        }
    }
    fn failed(&mut self, message: String) {
        tracing::warn!(station = ?self.current, error = %message, "radio playback failed");
        self.last_error = message.clone();
        let retried =
            self.retry_active || matches!(self.phase, Phase::Starting { retry: true, .. });
        self.gain = 0.;
        self.buffering = None;
        if retried && self.route + 1 < self.routes.len() {
            self.route += 1;
            self.route_fresh = true;
            self.retry_active = false;
            self.phase = Phase::RetryWait { until: self.now + self.cfg.retry_wait_before_restart };
            self.status = format!("Switching network to {}: {message}", super::proxy_description(self.routes[self.route].as_deref()));
        } else if retried {
            self.transition(self.next());
            self.status = format!("Retry failed; skipping: {message}");
        } else {
            self.phase = Phase::RetryWait {
                until: self.now + self.cfg.retry_wait_before_restart,
            };
            self.status = format!("Waiting before retry: {message}");
        }
    }
}

async fn signal(engine: &Engine, start: bool) {
    if !engine.cfg.sound_signals || engine.volume <= 0. { return; }
    let child = tokio::process::Command::new(&engine.cfg.python)
        .args(["-u", "-c", probe::BACKEND, "signal", if start { "cue-on" } else { "cue-off" }])
        .env("MOPYTUI_CUE_VOLUME", (engine.volume * 0.10).to_string())
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .kill_on_drop(true).spawn();
    if let Ok(mut child) = child {
        if tokio::time::timeout(Duration::from_secs(2), child.wait()).await.is_err() {
            let _ = child.kill().await; let _ = child.wait().await;
        }
    }
}

async fn close(deck: &mut Option<Deck>) {
    if let Some(old) = deck.take() {
        old.close().await;
    }
}
async fn launch(engine: &mut Engine, deck: &mut Option<Deck>, id: Option<String>, retry: bool) {
    close(deck).await;
    engine.artist.clear();
    engine.title.clear();
    engine.album.clear();
    engine.current = id;
    engine.retry_active = retry && !engine.route_fresh;
    let recovery = engine.retry_active;
    engine.route_fresh = false;
    engine.buffering = None;
    engine.gain = 0.;
    if !retry {
        engine.deadline = 0.;
        engine.route = 0;
        engine.routes = engine.cfg.routes();
    }
    let Some(entry) = engine
        .entries
        .iter()
        .find(|e| e.enabled && Some(e.station.id()) == engine.current.as_deref())
    else {
        engine.phase = Phase::Idle;
        engine.current = None;
        engine.status = "Stopped (no enabled stations)".into();
        signal(engine, false).await;
        return;
    };
    engine.phase = Phase::Starting {
        retry: recovery,
        until: engine.now + engine.cfg.retry_wait_after_restart + engine.cfg.buffer_seconds * 2.,
    };
    engine.last_audio = engine.now;
    engine.status = if retry {
        "Restarting stream"
    } else {
        "Connecting"
    }
    .into();
    match Deck::open(
        &engine.cfg.python,
        entry.station.stream_url(),
        engine.routes[engine.route].as_deref(),
        engine.cfg.buffer_seconds,
    ) {
        Ok(new) => *deck = Some(new),
        Err(e) => engine.failed(e.to_string()),
    }
}

async fn run(cfg: Config, mut rx: mpsc::UnboundedReceiver<Command>, state: Arc<Mutex<Snapshot>>) {
    let mut engine = Engine::new(cfg);
    if let Err(e) = engine.cfg.validate() {
        state.lock().unwrap().status = e.to_string();
        return;
    }
    let mut deck: Option<Deck> = None;
    let mut ticker = tokio::time::interval(Duration::from_millis(50));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut previous = tokio::time::Instant::now();
    loop {
        let mut acknowledgement = None;
        tokio::select! {
            command = rx.recv() => {
                let command = match command {
                    Some(Command::Remote(command, ack)) => { acknowledgement = Some(ack); Some(*command) }
                    other => other,
                };
                match command {
                    None | Some(Command::Shutdown) => break,
                    Some(Command::Replace(entries)) => engine.replace(entries),
                    Some(Command::Configure(cfg)) if cfg.validate().is_ok() => {
                        use rand::Rng;
                        let changed = (engine.cfg.rotation_min, engine.cfg.rotation_max) != (cfg.rotation_min, cfg.rotation_max);
                        engine.cfg = cfg;
                        if changed && engine.deadline > 0. {
                            let minutes = rand::rng().random_range(engine.cfg.rotation_min..=engine.cfg.rotation_max);
                            engine.deadline = engine.now + minutes as f64 * 60.;
                        }
                    },
                    Some(Command::Start) if engine.phase == Phase::Idle => {
                        let first = engine.first();
                        if first.is_some() { signal(&engine, true).await; }
                        launch(&mut engine, &mut deck, first, false).await;
                    }
                    Some(Command::StopNow) => {
                        let was_running = engine.phase != Phase::Idle;
                        close(&mut deck).await;
                        engine.phase = Phase::Idle;
                        engine.buffering = None;
                        engine.current = None;
                        engine.paused = false;
                        engine.gain = 0.;
                        engine.status = "Stopped".into();
                        if was_running { signal(&engine, false).await; }
                    }
                    Some(Command::Select(id)) => {
                        if engine.entries.iter().any(|e| e.enabled && e.station.id() == id) {
                            if engine.phase == Phase::Idle {
                                signal(&engine, true).await;
                                engine.paused = false;
                                launch(&mut engine, &mut deck, Some(id), false).await;
                            } else { engine.transition(Some(id)); }
                        }
                    }
                    Some(Command::Next) if engine.phase != Phase::Idle => engine.transition(engine.next()),
                    Some(Command::Previous) if engine.phase != Phase::Idle => engine.transition(engine.previous()),
                    Some(Command::SetPaused(paused)) if engine.phase != Phase::Idle => {
                        engine.paused = paused;
                        if let Some(d) = &mut deck { let _ = d.control(serde_json::json!({"paused": paused})).await; }
                    }
                    Some(Command::SetVolume(volume)) => engine.volume = volume.clamp(0., 1.),
                    Some(Command::Restart) if engine.phase != Phase::Idle => engine.transition(engine.current.clone()),
                    Some(Command::Stop) if engine.phase != Phase::Idle => engine.transition(None),
                    Some(Command::Pause) if engine.phase != Phase::Idle => {
                        engine.paused = !engine.paused;
                        if let Some(d) = &mut deck { let _ = d.control(serde_json::json!({"paused": engine.paused})).await; }
                    }
                    Some(Command::Volume(delta)) => engine.volume = (engine.volume + delta).clamp(0., 1.),
                    _ => {}
                }
                // Next / Stop / disabling current can leave pause mode.
                if !engine.paused {
                    if let Some(d) = &mut deck { let _ = d.control(serde_json::json!({"volume": engine.gain * engine.volume, "paused": false})).await; }
                }
            }
            _ = ticker.tick() => {
                let current_time = tokio::time::Instant::now();
                let dt = current_time.duration_since(previous).as_secs_f64();
                previous = current_time;
                {
                    if !engine.paused { engine.now += dt; }
                    let mut audio = false;
                    let mut error = None;
                    if let Some(d) = &mut deck {
                        while let Ok(event) = d.events.try_recv() {
                            match event { DeckEvent::Metadata(artist, title, album) => { engine.artist = artist; engine.title = title; engine.album = album; }, DeckEvent::Audio => audio = true,
                            DeckEvent::Buffering(percent) => {
                                if percent < 100 {
                                    if engine.buffering.is_none() { engine.buffer_since = engine.now; }
                                    engine.buffering = Some(percent);
                                    engine.status = format!("Buffering {percent}%");
                                } else if engine.buffering.take().is_some() {
                                    engine.last_audio = engine.now;
                                    if matches!(engine.phase, Phase::Playing | Phase::FadeIn { .. }) {
                                        engine.phase = Phase::FadeIn { since: engine.now };
                                        engine.gain = 0.;
                                        engine.status = "Fade in".into();
                                    }
                                }
                            }, DeckEvent::Error(e) => { if error.is_none() { error = Some(e); } } }
                        }
                    }
                    if engine.paused {
                        *state.lock().unwrap() = engine.snapshot();
                        continue;
                    }
                    if audio {
                        engine.last_audio = engine.now;
                        if engine.buffering.is_none() && matches!(engine.phase, Phase::Starting { .. }) {
                            use rand::Rng;
                            // An automatic retry of the same station preserves its timer.
                            // A first connection or manual transition gets a fresh interval.
                            if engine.deadline == 0. {
                                let minutes = rand::rng().random_range(engine.cfg.rotation_min..=engine.cfg.rotation_max);
                                engine.deadline = engine.now + minutes as f64 * 60.;
                            }
                            engine.audio_started = engine.now;
                            engine.phase = Phase::FadeIn { since: engine.now };
                            engine.status = "Fade in".into();
                        }
                        // One decoded packet is not a recovered connection. Keep the
                        // retry budget through the recovery window, including fade-in.
                        if engine.retry_active && engine.now - engine.audio_started >= engine.cfg.retry_wait_after_restart {
                            engine.retry_active = false;
                        }
                    }
                    if let Some(error) = error {
                        if matches!(engine.phase, Phase::Starting { .. } | Phase::FadeIn { .. } | Phase::Playing) {
                            engine.failed(error);
                            close(&mut deck).await;
                        }
                    }
                    if engine.buffering.is_some() {
                        if engine.now - engine.buffer_since > engine.cfg.buffer_seconds * 3. + engine.cfg.retry_wait_after_restart {
                            engine.failed("Buffer refill timed out".into());
                            close(&mut deck).await;
                        } else if matches!(engine.phase, Phase::Starting { .. } | Phase::Playing | Phase::FadeIn { .. }) {
                            *state.lock().unwrap() = engine.snapshot();
                            continue;
                        }
                    }
                    match engine.phase.clone() {
                        Phase::Idle => {},
                        Phase::Starting { until, .. } if engine.now >= until => {
                            engine.failed("No decoded audio".into()); close(&mut deck).await;
                        }
                        Phase::FadeOut { since, target, gain } => {
                            engine.gain = gain * (1. - (engine.now - since) / engine.cfg.fade_out).clamp(0., 1.);
                            if engine.now - since >= engine.cfg.fade_out {
                                launch(&mut engine, &mut deck, target, false).await;
                            }
                        }
                        Phase::RetryWait { until } if engine.now >= until => {
                            let id = engine.current.clone();
                            launch(&mut engine, &mut deck, id, true).await;
                        }
                        Phase::Playing | Phase::FadeIn { .. } => {
                            if engine.now - engine.last_audio >= engine.cfg.retry_wait_before_restart {
                                let reason = format!("No decoded audio for {:.1} s", engine.cfg.retry_wait_before_restart);
                                if engine.retry_active {
                                    engine.failed(reason);
                                } else {
                                    // The no-audio interval already constitutes the retry wait.
                                    engine.last_error = reason;
                                    tracing::warn!(station = ?engine.current, "radio decoded audio stalled");
                                    engine.phase = Phase::RetryWait { until: engine.now };
                                    engine.status = "Audio stalled; restarting".into();
                                }
                                engine.gain = 0.; close(&mut deck).await;
                            } else if engine.now >= engine.deadline {
                                engine.transition(engine.next());
                            } else if let Phase::FadeIn { since } = engine.phase {
                                engine.gain = ((engine.now - since) / engine.cfg.fade_in).clamp(0., 1.);
                                if engine.gain >= 1. { engine.phase = Phase::Playing; engine.status = "Playing".into(); }
                            }
                        }
                        _ => {},
                    }
                }
                if let Some(d) = &mut deck {
                    if let Err(e) = d.control(serde_json::json!({"volume": engine.gain * engine.volume})).await {
                        // The pipe reader owns backend failures: it retains the real
                        // decoder error. BrokenPipe here must not replace that error.
                        tracing::debug!(error = %e, "radio control pipe unavailable");
                    }
                }
            }
        }
        *state.lock().unwrap() = engine.snapshot();
        if let Some(ack) = acknowledgement {
            let _ = ack.send(());
        }
    }
    close(&mut deck).await;
    if engine.phase != Phase::Idle { signal(&engine, false).await; }
    *state.lock().unwrap() = Snapshot::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(id: &str) -> Entry {
        Entry {
            station: super::super::search::Station {
                stationuuid: id.into(),
                ..Default::default()
            },
            enabled: true,
        }
    }
    #[test]
    fn edits_preserve_current_and_deadline() {
        let mut e = Engine::new(Config::default());
        e.entries = vec![entry("a"), entry("b")];
        e.current = Some("a".into());
        e.phase = Phase::Playing;
        e.deadline = 1500.;
        e.replace(vec![entry("b"), entry("c"), entry("a")]);
        assert_eq!(e.phase, Phase::Playing);
        assert_eq!(e.deadline, 1500.);
        assert_eq!(e.next(), Some("b".into()));
    }
    #[test]
    fn removing_current_uses_old_successor_and_empty_stops() {
        let mut e = Engine::new(Config::default());
        e.entries = vec![entry("a"), entry("b"), entry("c")];
        e.current = Some("b".into());
        e.phase = Phase::Playing;
        e.replace(vec![entry("a"), entry("c")]);
        assert!(matches!(e.phase, Phase::FadeOut { target: Some(ref t), .. } if t == "c"));
        e.replace(vec![]);
        assert!(matches!(e.phase, Phase::FadeOut { target: None, .. }));
    }
    #[test]
    fn one_restart_then_skip() {
        let mut e = Engine::new(Config::default());
        e.entries = vec![entry("a"), entry("b")];
        e.current = Some("a".into());
        e.phase = Phase::Playing;
        e.now = 10.;
        e.failed("broken".into());
        assert_eq!(e.phase, Phase::RetryWait { until: 13. });
        e.phase = Phase::Starting {
            retry: true,
            until: 18.,
        };
        e.failed("broken again".into());
        assert!(matches!(e.phase, Phase::FadeOut { target: Some(ref t), .. } if t == "b"));
    }
    #[test]
    fn retry_that_dies_during_fade_in_skips_instead_of_restarting_forever() {
        let mut e = Engine::new(Config::default());
        e.entries = vec![entry("a"), entry("b")];
        e.current = Some("a".into());
        e.retry_active = true;
        e.phase = Phase::FadeIn { since: 10. };
        e.now = 11.;
        e.failed("connection died after one second of audio".into());
        assert!(matches!(e.phase, Phase::FadeOut { target: Some(ref t), .. } if t == "b"));
    }

    #[test]
    fn exhausted_retry_uses_alternative_once_before_skipping() {
        let mut e = Engine::new(Config { proxy: "socks5h://localhost:1080".into(), sound_signals: false, ..Default::default() });
        e.entries = vec![entry("a"), entry("b")];
        e.current = Some("a".into());
        e.phase = Phase::Playing;
        e.retry_active = true;
        e.failed("proxy unavailable".into());
        assert_eq!(e.route, 1);
        assert!(matches!(e.phase, Phase::RetryWait { .. }));
        assert_eq!(e.snapshot().network, "direct");
        e.phase = Phase::Playing; e.retry_active = true;
        e.failed("direct unavailable".into());
        assert!(matches!(e.phase, Phase::FadeOut { target: Some(ref id), .. } if id == "b"));
    }
    #[test]
    fn disabled_entries_are_skipped() {
        let mut e = Engine::new(Config::default());
        let mut disabled = entry("b");
        disabled.enabled = false;
        e.entries = vec![entry("a"), disabled, entry("c")];
        e.current = Some("a".into());
        assert_eq!(e.next(), Some("c".into()));
    }
}

#[cfg(all(test, unix))]
mod actor_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    async fn wait_for(handle: &Handle, pred: impl Fn(&Snapshot) -> bool) -> Snapshot {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let snapshot = handle.snapshot();
                if pred(&snapshot) {
                    return snapshot;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("actor did not reach expected state")
    }
    #[tokio::test]
    async fn actor_retries_once_pauses_and_switches_without_mopidy() {
        let root = std::env::temp_dir().join(format!(
            "mopytui-actor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let script = root.join("fake-python");
        std::fs::write(
            &script,
            r#"#!/usr/bin/python3
import sys, json, select, os, time
from pathlib import Path
url = sys.argv[-2]
with open(Path(__file__).with_name('starts'), 'a') as log: log.write(url + '\n')
pending = b''
paused = False
print(json.dumps({'event':'metadata','artist':'Test artist','title':url+' track','album':''}), flush=True)
while True:
    if url != 'broken' and not paused:
        print(json.dumps({'event':'audio'}), flush=True)
    ready, _, _ = select.select([sys.stdin.buffer], [], [], 0.025)
    if ready:
        data = os.read(sys.stdin.fileno(), 4096)
        if not data: break
        pending += data
        while b'\n' in pending:
            line, pending = pending.split(b'\n', 1)
            cmd = json.loads(line)
            if cmd.get('stop'): sys.exit(0)
            if 'paused' in cmd: paused = cmd['paused']
"#,
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cfg = Config {
            python: script.to_string_lossy().into(),
            fade_in: 0.1,
            fade_out: 0.1,
            buffer_seconds: 0.,
            sound_signals: false,
            retry_wait_before_restart: 0.15,
            retry_wait_after_restart: 0.15,
            ..Config::default()
        };
        let mut handle = Handle::new(cfg.clone());
        let entry = |id: &str| Entry {
            station: super::super::search::Station {
                stationuuid: id.into(),
                url: id.into(),
                name: id.into(),
                ..Default::default()
            },
            enabled: true,
        };
        handle.send(Command::Replace(vec![
            entry("broken"),
            entry("good"),
            entry("next"),
        ]));
        handle.send(Command::Start);
        let playing = wait_for(&handle, |s| {
            s.current.as_deref() == Some("good") && s.status == "Playing"
        })
        .await;
        assert!((23 * 60 - 1..=37 * 60).contains(&playing.remaining));
        assert_eq!(playing.artist, "Test artist");
        assert_eq!(playing.title, "good track");
        let remote = handle.control();
        remote
            .send_confirmed(Command::SetVolume(0.35))
            .await
            .unwrap();
        assert_eq!(remote.snapshot().volume, 0.35);
        remote
            .send_confirmed(Command::SetPaused(true))
            .await
            .unwrap();
        assert!(remote.snapshot().paused);
        let adjusted = Config {
            rotation_min: 1,
            rotation_max: 1,
            ..cfg
        };
        remote
            .send_confirmed(Command::Configure(adjusted))
            .await
            .unwrap();
        assert_eq!(remote.snapshot().remaining, 60);
        assert!(remote.snapshot().paused);
        assert_eq!(remote.snapshot().current.as_deref(), Some("good"));

        remote
            .send_confirmed(Command::SetPaused(false))
            .await
            .unwrap();
        assert!(!remote.snapshot().paused);
        let starts = std::fs::read_to_string(root.join("starts")).unwrap();
        assert_eq!(starts.lines().filter(|s| *s == "broken").count(), 2);
        handle.send(Command::Pause);
        let paused = wait_for(&handle, |s| s.paused).await;
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert_eq!(paused.remaining, handle.snapshot().remaining);
        handle.send(Command::Next);
        wait_for(&handle, |s| {
            s.current.as_deref() == Some("next") && s.status == "Playing" && s.title == "next track"
        })
        .await;
        remote.send_confirmed(Command::Previous).await.unwrap();
        wait_for(&handle, |s| {
            s.current.as_deref() == Some("good") && s.status == "Playing"
        })
        .await;
        handle.send(Command::Stop);
        wait_for(&handle, |s| !s.running).await;
        remote.send_confirmed(Command::Select("next".into())).await.unwrap();
        wait_for(&handle, |s| s.current.as_deref() == Some("next") && s.status == "Playing").await;
        remote.send_confirmed(Command::SetPaused(true)).await.unwrap();
        remote.send_confirmed(Command::Select("good".into())).await.unwrap();
        wait_for(&handle, |s| s.current.as_deref() == Some("good") && s.status == "Playing" && !s.paused).await;
        remote.send_confirmed(Command::StopNow).await.unwrap();
        assert!(!remote.snapshot().running);
        assert!(remote.snapshot().current.is_none());
        handle.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[tokio::test]
    async fn gst_error_followed_by_eof_is_reported_once() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let stdout =
            b"{\"event\":\"audio\"}\n{\"event\":\"error\",\"message\":\"HTTP connection reset\"}\n";
        read_events(&stdout[..], &b"GStreamer diagnostic details\n"[..], tx).await;
        assert!(matches!(rx.recv().await, Some(DeckEvent::Audio)));
        assert!(
            matches!(rx.recv().await, Some(DeckEvent::Error(e)) if e == "HTTP connection reset")
        );
        assert!(
            rx.recv().await.is_none(),
            "EOF must not emit a second error"
        );
    }
    #[tokio::test]
    async fn stderr_survives_crash_without_json_error() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        read_events(
            &b""[..],
            &b"Traceback:\nRuntimeError: device unavailable\n"[..],
            tx,
        )
        .await;
        assert!(
            matches!(rx.recv().await, Some(DeckEvent::Error(e)) if e.contains("device unavailable"))
        );
    }
}
