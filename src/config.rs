use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub radio: mopytui::radio::Config,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_mpd_port")]
    pub mpd_port: u16,
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub mpris: bool,
    /// Path to a named pipe / file with raw s16le stereo PCM at 44.1 kHz.
    /// When set and reachable, the spectrum visualizer switches from the
    /// pseudo oscillators to a real FFT of the live audio. Only useful when
    /// mopidy runs on the same machine as mopytui (or the FIFO is bridged
    /// somehow). Leave empty to keep pseudo-spectrum.
    #[serde(default)]
    pub audio_pipe: Option<String>,
    #[serde(default)]
    pub lastfm_api_key: Option<String>,
    #[serde(default)]
    pub fanart_api_key: Option<String>,
    #[serde(default)]
    pub discogs_token: Option<String>,
}

fn default_host() -> String { "127.0.0.1".into() }
fn default_mpd_port() -> u16 { 6600 }
fn default_http_port() -> u16 { 6680 }
fn default_theme() -> String { "midnight".into() }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            radio: mopytui::radio::Config::default(),
            host: default_host(),
            mpd_port: default_mpd_port(),
            http_port: default_http_port(),
            theme: default_theme(),
            mpris: false,
            audio_pipe: None,
            lastfm_api_key: None,
            fanart_api_key: None,
            discogs_token: None,
        }
    }
}

const TEMPLATE: &str = r#"# mopytui config
host = "127.0.0.1"
http_port = 6680
mpd_port = 6600

# midnight | soft-dark | daylight | solar
theme = "midnight"

# Enable MPRIS integration (Linux only — no-op elsewhere).
mpris = false

# Live FFT spectrum visualizer (optional). Only works when mopidy runs on
# the same machine as mopytui — set the path to a named pipe / file that
# carries raw s16le stereo PCM at 44.1 kHz. Leave commented for pseudo
# spectrum (no audio analysis).
#
# Linux + PipeWire (no mopidy config needed — just tap the sink monitor):
#     mkfifo /tmp/mopidy.fifo
#     pw-record --target @DEFAULT_AUDIO_SINK@.monitor \
#       --format=s16 --rate=44100 --channels=2 - > /tmp/mopidy.fifo &
#
# macOS local + DAC (add a `tee` to mopidy.conf — see README for the safe
# bit-perfect pipeline):
#     mkfifo /tmp/mopidy.fifo
#
# audio_pipe = "/tmp/mopidy.fifo"

# Optional API keys for richer metadata.
# last.fm:   https://www.last.fm/api/account/create
# fanart.tv: https://fanart.tv/get-an-api-key/
# discogs:   https://www.discogs.com/settings/developers
lastfm_api_key = ""
fanart_api_key = ""
discogs_token = ""

[radio]
# Optional proxy for radio search, probes and playback (Mopidy stays direct).
# proxy = "socks5h://127.0.0.1:1080"
preferred_network = "proxy"
buffer_seconds = 3.0
sound_signals = true
rotation_min = 23
rotation_max = 37
fade_out = 1.5
fade_in = 2.5
retry_wait_before_restart = 3.0
retry_wait_after_restart = 5.0
station_probe_timeout = 5.0
python = "python3"
"#;

pub fn config_path() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "mopytui")?;
    Some(dirs.config_dir().join("config.toml"))
}

pub fn cache_dir() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "mopytui")?;
    let dir = dirs.cache_dir().to_path_buf();
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

pub fn log_path() -> Option<PathBuf> {
    Some(cache_dir()?.join("mopytui.log"))
}

pub fn load_or_template() -> AppConfig {
    let Some(path) = config_path() else {
        return AppConfig::default();
    };
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, TEMPLATE);
        eprintln!("wrote template config at {}", path.display());
        return AppConfig::default();
    }
    match read_config(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config error: {e} — using defaults");
            AppConfig::default()
        }
    }
}

fn read_config(path: &Path) -> Result<AppConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    Ok(toml::from_str::<AppConfig>(&raw)?)
}

/// Persist only the network defaults, retaining comments and unrelated settings.
pub fn save_radio_network(cfg: &mopytui::radio::Config) -> Result<()> {
    let path = config_path().context("config directory unavailable")?;
    save_radio_network_at(&path, cfg)
}
fn save_radio_network_at(path: &Path, cfg: &mopytui::radio::Config) -> Result<()> {
    use std::io::Write;
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => TEMPLATE.into(),
        Err(e) => return Err(e.into()),
    };
    let _: AppConfig = toml::from_str(&raw)?;
    let mut document: toml_edit::DocumentMut = raw.parse()?;
    document["radio"]["preferred_network"] = toml_edit::value(match cfg.preferred_network {
        mopytui::radio::Network::Direct => "direct", mopytui::radio::Network::Proxy => "proxy",
    });
    document["radio"]["buffer_seconds"] = toml_edit::value(cfg.buffer_seconds);
    document["radio"]["sound_signals"] = toml_edit::value(cfg.sound_signals);
    let parent = path.parent().context("config parent missing")?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".radio-network-{}-{}.tmp", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));
    let result = (|| -> Result<()> {
        let mut options = std::fs::OpenOptions::new(); options.write(true).create_new(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        let mut file = options.open(&temporary)?;
        file.write_all(document.to_string().as_bytes())?; file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temporary); }
    result
}

#[cfg(test)]
mod network_tests {
    use super::*;
    #[test]
    fn saves_network_defaults_without_losing_comments_or_other_keys() {
        let path = std::env::temp_dir().join(format!("mopytui-network-{}-{}.toml", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::write(&path, "# keep this\nhost = \"music.local\"\n[radio]\nproxy = \"socks5h://localhost:1080\" # keep proxy\nrotation_min = 23\n").unwrap();
        let cfg = mopytui::radio::Config { preferred_network: mopytui::radio::Network::Direct, buffer_seconds: 5., sound_signals: false, ..Default::default() };
        save_radio_network_at(&path, &cfg).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# keep this") && text.contains("# keep proxy"));
        let saved = read_config(&path).unwrap();
        assert_eq!(saved.host, "music.local");
        assert_eq!(saved.radio.proxy, "socks5h://localhost:1080");
        assert_eq!(saved.radio.preferred_network, mopytui::radio::Network::Direct);
        assert_eq!(saved.radio.buffer_seconds, 5.);
        assert!(!saved.radio.sound_signals);
        std::fs::write(&path, "invalid [toml").unwrap();
        assert!(save_radio_network_at(&path, &cfg).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid [toml");
        std::fs::remove_file(path).unwrap();
    }
}
