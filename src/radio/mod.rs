pub mod collections;
pub mod player;
pub mod probe;
pub mod remote;
pub mod search;

use search::Station;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::time::Duration;
use tokio::sync::mpsc;

/// Connection label deliberately excludes proxy credentials.
pub fn proxy_description(proxy: Option<&str>) -> String {
    match proxy {
        Some(value) => match reqwest::Url::parse(value) {
            Ok(url) => format!(
                "proxy {}://{}{}",
                url.scheme(),
                url.host_str().unwrap_or("?"),
                url.port().map(|p| format!(":{p}")).unwrap_or_default()
            ),
            Err(_) => "proxy: invalid URL".into(),
        },
        None => "direct".into(),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    Direct,
    #[default]
    Proxy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub rotation_min: u64,
    pub rotation_max: u64,
    pub fade_out: f64,
    pub fade_in: f64,
    pub retry_wait_before_restart: f64,
    pub retry_wait_after_restart: f64,
    pub station_probe_timeout: f64,
    pub python: String,
    pub servers: Vec<String>,
    pub proxy: String,
    pub preferred_network: Network,
    pub buffer_seconds: f64,
    pub sound_signals: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            rotation_min: 23,
            rotation_max: 37,
            fade_out: 1.5,
            fade_in: 2.5,
            retry_wait_before_restart: 3.,
            retry_wait_after_restart: 5.,
            station_probe_timeout: 5.,
            python: "python3".into(),
            proxy: String::new(),
            preferred_network: Network::Proxy,
            buffer_seconds: 3.,
            sound_signals: true,
            servers: vec!["https://de1.api.radio-browser.info".into()],
        }
    }
}
impl Config {
    pub fn effective_proxy(&self) -> Option<String> {
        std::env::var("MOPYTUI_RADIO_PROXY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| (!self.proxy.trim().is_empty()).then(|| self.proxy.clone()))
    }

    /// Routes are explicit: direct must never inherit HTTP_PROXY/ALL_PROXY.
    pub fn routes(&self) -> Vec<Option<String>> {
        match self.effective_proxy() {
            Some(proxy) if self.preferred_network == Network::Proxy => vec![Some(proxy), None],
            Some(proxy) => vec![None, Some(proxy)],
            None => vec![None],
        }
    }
    pub fn preferred_description(&self) -> String {
        proxy_description(self.routes()[0].as_deref())
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.rotation_min > 0
                && self.rotation_max >= self.rotation_min
                && self.rotation_max <= 10080,
            "radio rotation must satisfy 1 <= min <= max <= 10080 minutes"
        );
        anyhow::ensure!(self.buffer_seconds.is_finite() && (0.0..=30.0).contains(&self.buffer_seconds), "radio buffer must be between 0 and 30 seconds");
        for value in [
            self.fade_in,
            self.fade_out,
            self.retry_wait_before_restart,
            self.retry_wait_after_restart,
            self.station_probe_timeout,
        ] {
            anyhow::ensure!(
                value.is_finite() && value > 0. && value <= 300.,
                "radio times must be between 0 and 300 seconds"
            );
        }
        if !self.proxy.trim().is_empty() {
            let url = reqwest::Url::parse(&self.proxy)?;
            anyhow::ensure!(
                matches!(url.scheme(), "socks5h" | "socks5" | "http" | "https")
                    && url.host_str().is_some(),
                "radio.proxy must be an HTTP(S) or SOCKS5 proxy URL"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub station: Station,
    pub enabled: bool,
}
#[derive(Clone, Debug, Default)]
pub enum Check {
    #[default]
    Unchecked,
    Checking,
    Ok,
    Dead(String),
}
#[derive(Clone, Debug)]
pub struct ResultRow {
    pub station: Station,
    pub selected: bool,
    pub check: Check,
}
enum Event {
    Search(u64, Result<Vec<Station>, String>),
    SearchProgress(u64, String),
    Probe(String, Result<(), String>),
    Paused(u64, Result<(), String>),
}

pub struct Controller {
    pub config: Config,
    pub collections: collections::Browser,
    pub query: String,
    pub editing: bool,
    pub settings: Option<(Config, usize)>,
    pub search_view: bool,
    pub results: Vec<ResultRow>,
    pub cursor: usize,
    pub loop_cursor: usize,
    pub entries: Vec<Entry>,
    pub message: String,
    pub searching: bool,
    pub search_started: Option<std::time::Instant>,
    pub pending_start: bool,
    pub ready_play: bool,
    pub play_target: Option<String>,
    pub result_offset: usize,
    pub loop_offset: usize,
    start_generation: u64,
    start_task: Option<tokio::task::JoinHandle<()>>,
    pub snapshot: player::Snapshot,
    tx: mpsc::UnboundedSender<Event>,
    rx: mpsc::UnboundedReceiver<Event>,
    player: player::Handle,
    generation: u64,
    search_task: Option<tokio::task::JoinHandle<()>>,
    probes: Vec<tokio::task::JoinHandle<()>>,
    probe_slots: std::sync::Arc<tokio::sync::Semaphore>,
}
impl Controller {
    pub fn new(config: Config) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let player = player::Handle::new(config.clone());
        Self {
            config,
            collections: collections::Browser::default(),
            query: String::new(),
            editing: false,
            settings: None,
            search_view: true,
            results: vec![],
            cursor: 0,
            loop_cursor: 0,
            entries: vec![],
            message: String::new(),
            searching: false,
            search_started: None,
            pending_start: false,
            ready_play: false,
            play_target: None,
            result_offset: 0,
            loop_offset: 0,
            start_generation: 0,
            start_task: None,
            snapshot: player::Snapshot::default(),
            tx,
            rx,
            player,
            generation: 0,
            search_task: None,
            probes: vec![],
            probe_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
        }
    }
    pub fn open_collections(&mut self) {
        match collections::read(&collections::path()) {
            Ok(items) => {
                self.collections = collections::Browser {
                    items,
                    visible: true,
                    ..Default::default()
                };
                self.message.clear();
            }
            Err(e) => self.message = e.to_string(),
        }
    }
    pub fn save_collection(&mut self) {
        let name = self
            .collections
            .input
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_owned();
        let result = (|| -> anyhow::Result<()> {
            collections::valid_name(&name)?;
            let mut items = self.collections.items.clone();
            let index = self.collections.cursor;
            anyhow::ensure!(
                !items
                    .iter()
                    .enumerate()
                    .any(|(i, c)| c.name == name && (!self.collections.rename || i != index)),
                "Name already exists; choose another name"
            );
            if self.collections.rename {
                let c = items
                    .get_mut(index)
                    .ok_or_else(|| anyhow::anyhow!("No collection selected"))?;
                c.name = name;
            } else {
                anyhow::ensure!(
                    !self.entries.is_empty(),
                    "Add stations to the loop before saving"
                );
                items.push(collections::Collection::capture(
                    name,
                    &self.entries,
                    &self.config,
                ));
            }
            collections::write(&collections::path(), &items)?;
            self.collections.items = items;
            if !self.collections.rename {
                self.collections.cursor = self.collections.items.len().saturating_sub(1);
            }
            self.collections.input = None;
            self.message = "Collection saved".into();
            Ok(())
        })();
        if let Err(e) = result {
            self.message = e.to_string();
        }
    }
    pub fn load_collection(&mut self) {
        if self.snapshot.running || self.pending_start {
            self.message = "Stop Radio Loop before loading a collection".into();
            return;
        }
        if let Some(c) = self.collections.items.get(self.collections.cursor).cloned() {
            match c.config(&self.config) {
                Ok(cfg) => {
                    self.config = cfg.clone();
                    self.command(player::Command::Configure(cfg));
                    self.entries = c.entries;
                    self.loop_cursor = 0;
                    self.sync();
                    self.collections.visible = false;
                    self.editing = false;
                    self.search_view = false;
                    self.message = format!("Loaded {} · Enter starts the loop", c.name);
                }
                Err(e) => self.message = e.to_string(),
            }
        }
    }
    pub fn update_collection(&mut self) {
        let mut items = self.collections.items.clone();
        if let Some(c) = items.get_mut(self.collections.cursor) {
            if self.entries.is_empty() {
                self.message = "Current loop is empty".into();
            } else {
                *c = collections::Collection::capture(c.name.clone(), &self.entries, &self.config);
                match collections::write(&collections::path(), &items) {
                    Ok(()) => {
                        self.collections.items = items;
                        self.message = "Collection updated".into();
                    }
                    Err(e) => self.message = e.to_string(),
                }
            }
        }
        self.collections.update_confirm = false;
    }
    pub fn delete_collection(&mut self) {
        let mut items = self.collections.items.clone();
        if self.collections.cursor < items.len() {
            items.remove(self.collections.cursor);
        }
        match collections::write(&collections::path(), &items) {
            Ok(()) => {
                self.collections.items = items;
                self.collections.cursor = self
                    .collections
                    .cursor
                    .min(self.collections.items.len().saturating_sub(1));
                self.message = "Collection deleted".into();
            }
            Err(e) => self.message = e.to_string(),
        }
        self.collections.delete_confirm = false;
    }
    pub fn search(&mut self) {
        if let Some(t) = self.search_task.take() {
            t.abort();
        }
        self.generation += 1;
        let generation = self.generation;
        let (tx, query, servers) = (
            self.tx.clone(),
            self.query.clone(),
            self.config.servers.clone(),
        );
        let cfg = self.config.clone();
        self.searching = true;
        self.search_started = Some(std::time::Instant::now());
        self.message = "Searching…".into();
        self.search_task = Some(tokio::spawn(async move {
            let progress_tx = tx.clone();
            let result = search::search_with_progress(&query, &servers, &cfg, move |message| {
                let _ = progress_tx.send(Event::SearchProgress(generation, message));
            }).await
                .map_err(|e| e.to_string());
            let _ = tx.send(Event::Search(generation, result));
        }));
    }
    /// Add only decoded stations; an Enter intent survives the asynchronous probe.
    pub fn choose_result(&mut self, play: bool) {
        let Some(row) = self.results.get(self.cursor) else { return; };
        let id = row.station.id().to_owned();
        if play { self.play_target = Some(id.clone()); }
        if matches!(row.check, Check::Ok) {
            self.insert_verified(&id);
            if play { self.ready_play = true; }
        } else if !matches!(row.check, Check::Checking) {
            self.results[self.cursor].selected = false;
            self.toggle_result();
        } else { self.results[self.cursor].selected = true; }
    }
    fn insert_verified(&mut self, id: &str) {
        let Some(row) = self.results.iter().find(|r| r.station.id() == id && matches!(r.check, Check::Ok)) else { return; };
        if let Some(entry) = self.entries.iter_mut().find(|e| e.station.id() == id) {
            entry.enabled = true;
        } else {
            self.entries.push(Entry { station: row.station.clone(), enabled: true });
        }
        self.sync();
    }
    pub fn choose_loop(&mut self) -> bool {
        let Some(entry) = self.entries.get_mut(self.loop_cursor) else { return false; };
        entry.enabled = true;
        self.play_target = Some(entry.station.id().to_owned());
        self.sync();
        true
    }
    pub async fn stop_for_music(&mut self) -> anyhow::Result<()> {
        self.command(player::Command::Stop);
        self.control().send_confirmed(player::Command::StopNow).await?;
        self.snapshot = self.player.snapshot();
        Ok(())
    }
    pub fn toggle_result(&mut self) {
        let Some(row) = self.results.get_mut(self.cursor) else {
            return;
        };
        row.selected = !row.selected;
        if row.selected && !matches!(row.check, Check::Ok | Check::Checking) {
            row.check = Check::Checking;
            let (station, tx, cfg) = (row.station.clone(), self.tx.clone(), self.config.clone());
            let slots = self.probe_slots.clone();
            self.probes.push(tokio::spawn(async move {
                let Ok(_permit) = slots.acquire_owned().await else {
                    return;
                };
                let result = probe::probe_with_config(&cfg, station.stream_url())
                .await
                .map_err(|e| e.to_string());
                let _ = tx.send(Event::Probe(station.id().into(), result));
            }));
        }
    }
    pub fn poll(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::SearchProgress(generation, message) if generation == self.generation && self.searching => self.message = message,
                Event::Search(generation, result) if generation == self.generation => {
                    self.searching = false;
                    self.search_started = None;
                    match result {
                        Ok(stations) => {
                            self.results = stations
                                .into_iter()
                                .map(|station| ResultRow {
                                    station,
                                    selected: false,
                                    check: Check::Unchecked,
                                })
                                .collect();
                            self.cursor = 0;
                            self.message = format!(
                                "{} results. Space: select and verify audio",
                                self.results.len()
                            );
                        }
                        Err(e) => self.message = e,
                    }
                }
                Event::Paused(generation, result) if generation == self.start_generation => {
                    self.pending_start = false;
                    match result {
                        Ok(()) if self.has_stations() => self.start(),
                        Ok(()) => self.message = "No enabled stations".into(),
                        Err(e) => {
                            self.message =
                                format!("Cannot pause Mopidy; radio was not started: {e}")
                        }
                    }
                }
                Event::Probe(id, result) => {
                    for row in &mut self.results {
                        if row.station.id() == id && matches!(row.check, Check::Checking) {
                            row.check = match &result {
                                Ok(()) => Check::Ok,
                                Err(e) => Check::Dead(e.clone()),
                            };
                        }
                    }
                    if result.is_ok() && self.results.iter().any(|r| r.station.id() == id && r.selected) {
                        self.insert_verified(&id);
                        if self.play_target.as_deref() == Some(&id) { self.ready_play = true; }
                    } else if result.is_err() && self.play_target.as_deref() == Some(&id) {
                        self.play_target = None;
                    }
                }
                _ => {}
            }
        }
        if self.searching && (self.search_started.is_some_and(|t| t.elapsed().as_secs() >= 30)) {
            if let Some(task) = self.search_task.take() { task.abort(); }
            self.generation += 1;
            self.searching = false;
            self.search_started = None;
            self.message = "Radio Browser search stopped: timeout or failed worker. Saved stations remain available.".into();
        }
        self.probes.retain(|t| !t.is_finished());
        self.snapshot = self.player.snapshot();
    }
    pub fn add_selected(&mut self) -> bool {
        if self
            .results
            .iter()
            .any(|r| r.selected && matches!(r.check, Check::Checking))
        {
            self.message = "Audio check is still running; press Enter when it finishes".into();
            return false;
        }
        let mut count = 0;
        for row in &mut self.results {
            if row.selected && matches!(row.check, Check::Ok) {
                if !self
                    .entries
                    .iter()
                    .any(|e| e.station.id() == row.station.id())
                {
                    self.entries.push(Entry {
                        station: row.station.clone(),
                        enabled: true,
                    });
                    count += 1;
                }
                row.selected = false;
            }
        }
        self.sync();
        self.message = format!("Added {count} stations; unavailable streams were excluded");
        self.has_stations()
    }
    pub fn control(&self) -> player::Control {
        self.player.control()
    }
    pub fn has_stations(&self) -> bool {
        self.entries.iter().any(|e| e.enabled)
    }
    pub fn request_start(
        &mut self,
        pause: impl std::future::Future<Output = Result<(), String>> + Send + 'static,
    ) {
        if self.pending_start || !self.has_stations() {
            return;
        }
        self.ready_play = false;
        self.pending_start = true;
        self.message = "Pausing Mopidy…".into();
        self.start_generation += 1;
        let generation = self.start_generation;
        let tx = self.tx.clone();
        self.start_task = Some(tokio::spawn(async move {
            let result = pause.await;
            let _ = tx.send(Event::Paused(generation, result));
        }));
    }
    fn start(&mut self) {
        self.message = "Radio started".into();
        self.player.send(match self.play_target.take() { Some(id) => player::Command::Select(id), None => player::Command::Start });
        self.snapshot.running = true;
    }
    pub fn command(&mut self, cmd: player::Command) {
        if matches!(cmd, player::Command::Stop) {
            self.start_generation += 1;
            self.pending_start = false;
            self.ready_play = false;
            self.play_target = None;
            if let Some(task) = self.start_task.take() {
                task.abort();
            }
        }
        self.player.send(cmd);
    }
    pub fn sync(&self) {
        self.player
            .send(player::Command::Replace(self.entries.clone()));
    }
    pub async fn shutdown(&mut self) {
        if let Some(task) = self.start_task.take() {
            task.abort();
        }
        if let Some(task) = self.search_task.take() {
            task.abort();
        }
        for task in self.probes.drain(..) {
            task.abort();
        }
        self.player.shutdown().await;
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(t) = &self.search_task {
            t.abort();
        }
        if let Some(t) = &self.start_task {
            t.abort();
        }
        for task in &self.probes {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn loading_collection_is_stopped_and_keeps_connection_config() {
        let cfg = Config {
            proxy: "socks5h://localhost:1080".into(),
            ..Default::default()
        };
        let mut radio = Controller::new(cfg.clone());
        let saved = Config {
            rotation_min: 2,
            rotation_max: 4,
            fade_out: 0.5,
            ..Default::default()
        };
        let entries = vec![Entry {
            station: Station {
                stationuuid: "saved".into(),
                url: "https://example.test/live".into(),
                ..Default::default()
            },
            enabled: false,
        }];
        radio
            .collections
            .items
            .push(collections::Collection::capture(
                "Work".into(),
                &entries,
                &saved,
            ));
        radio.snapshot.running = true;
        radio.load_collection();
        assert!(radio.entries.is_empty());
        radio.snapshot.running = false;
        radio.editing = true;
        radio.load_collection();
        assert_eq!(radio.entries.len(), 1);
        assert!(!radio.entries[0].enabled);
        assert_eq!(radio.config.proxy, cfg.proxy);
        assert_eq!(radio.config.rotation_min, 2);
        assert!(!radio.snapshot.running && !radio.pending_start && !radio.editing);
        radio.shutdown().await;
    }
    #[test]
    fn proxy_configuration_is_optional_and_validated() {
        let config: Config = toml::from_str("proxy = \"socks5h://127.0.0.1:1080\"").unwrap();
        assert_eq!(config.proxy, "socks5h://127.0.0.1:1080");
        assert!(config.validate().is_ok());
        assert!(
            Config {
                proxy: "file:///tmp/proxy".into(),
                ..config
            }
            .validate()
            .is_err()
        );
        assert!(Config::default().proxy.is_empty());
    }
    #[tokio::test]
    async fn probe_completion_adds_and_starts_only_the_requested_station() {
        let mut r = Controller::new(Config::default());
        for id in ["first", "chosen"] {
            r.results.push(ResultRow { station: Station { stationuuid: id.into(), ..Default::default() }, selected: true, check: Check::Checking });
        }
        r.cursor = 1;
        r.choose_result(true);
        assert!(!r.ready_play && r.entries.is_empty());
        r.tx.send(Event::Probe("first".into(), Ok(()))).unwrap(); r.poll();
        assert_eq!(r.entries.len(), 1); assert!(!r.ready_play);
        r.tx.send(Event::Probe("chosen".into(), Ok(()))).unwrap(); r.poll();
        assert_eq!(r.entries.len(), 2); assert!(r.ready_play);
        assert_eq!(r.play_target.as_deref(), Some("chosen"));
        r.command(player::Command::Stop);
        assert!(!r.ready_play && r.play_target.is_none());
        r.shutdown().await;
    }
    #[test]
    fn network_preference_orders_explicit_routes() {
        let mut cfg = Config { proxy: "socks5h://localhost:1080".into(), ..Default::default() };
        assert_eq!(cfg.routes(), vec![Some(cfg.proxy.clone()), None]);
        cfg.preferred_network = Network::Direct;
        assert_eq!(cfg.routes(), vec![None, Some(cfg.proxy.clone())]);
        cfg.buffer_seconds = 31.; assert!(cfg.validate().is_err());
        cfg.buffer_seconds = 0.; assert!(cfg.validate().is_ok());
    }
    #[tokio::test]
    async fn watchdog_clears_stuck_search_and_ignores_late_replies() {
        let mut radio = Controller::new(Config::default());
        radio.searching = true;
        radio.search_started = Some(std::time::Instant::now() - Duration::from_secs(31));
        let old_generation = radio.generation;
        radio.poll();
        assert!(!radio.searching);
        assert!(radio.message.contains("timeout"));
        radio.tx.send(Event::Search(old_generation, Ok(vec![]))).unwrap();
        radio.tx.send(Event::SearchProgress(old_generation, "Searching…".into())).unwrap();
        radio.poll();
        assert!(radio.message.contains("timeout"));
        radio.shutdown().await;
    }
    #[test]
    fn defaults_and_validation() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!((c.rotation_min, c.rotation_max), (23, 37));
        assert_eq!((c.fade_out, c.fade_in), (1.5, 2.5));
        assert!(c.validate().is_ok());
        assert!(
            Config {
                rotation_min: 40,
                ..c.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Config {
                fade_in: f64::NAN,
                ..c.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Config {
                rotation_min: 0,
                ..c
            }
            .validate()
            .is_err()
        );
    }
    #[tokio::test]
    async fn start_requires_successful_pause_and_stop_cancels_pending_start() {
        let mut radio = Controller::new(Config::default());
        radio.entries.push(Entry {
            station: Station {
                stationuuid: "a".into(),
                ..Default::default()
            },
            enabled: true,
        });
        radio.request_start(async { Err("server offline".into()) });
        tokio::task::yield_now().await;
        radio.poll();
        assert!(!radio.pending_start && !radio.snapshot.running);
        assert!(radio.message.contains("server offline"));
        radio.request_start(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(())
        });
        assert!(radio.pending_start);
        radio.command(player::Command::Stop);
        tokio::task::yield_now().await;
        radio.poll();
        assert!(!radio.pending_start && !radio.snapshot.running);
        radio.shutdown().await;
    }
    #[tokio::test]
    async fn only_verified_selected_stations_enter_loop() {
        let mut radio = Controller::new(Config::default());
        for (id, check) in [
            ("ok", Check::Ok),
            ("bad", Check::Dead("decode failed".into())),
            ("unknown", Check::Unchecked),
        ] {
            radio.results.push(ResultRow {
                station: Station {
                    stationuuid: id.into(),
                    ..Default::default()
                },
                selected: true,
                check,
            });
        }
        assert!(radio.add_selected());
        assert_eq!(radio.entries.len(), 1);
        assert_eq!(radio.entries[0].station.id(), "ok");
        radio.results[0].selected = true;
        radio.add_selected();
        assert_eq!(radio.entries.len(), 1);
        radio.shutdown().await;
    }
}
