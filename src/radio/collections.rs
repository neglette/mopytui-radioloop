//! Named station sets; credentials/network settings are deliberately not saved.
use super::{Config, Entry};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Collection {
    pub name: String,
    pub entries: Vec<Entry>,
    pub rotation_min: u64,
    pub rotation_max: u64,
    pub fade_out: f64,
    pub fade_in: f64,
}
impl Collection {
    pub fn capture(name: String, entries: &[Entry], cfg: &Config) -> Self {
        Self {
            name,
            entries: entries.to_vec(),
            rotation_min: cfg.rotation_min,
            rotation_max: cfg.rotation_max,
            fade_out: cfg.fade_out,
            fade_in: cfg.fade_in,
        }
    }
    pub fn config(&self, base: &Config) -> Result<Config> {
        let mut cfg = base.clone();
        cfg.rotation_min = self.rotation_min;
        cfg.rotation_max = self.rotation_max;
        cfg.fade_out = self.fade_out;
        cfg.fade_in = self.fade_in;
        cfg.validate()?;
        Ok(cfg)
    }
}
#[derive(Default, Serialize, Deserialize)]
struct File {
    version: u32,
    collections: Vec<Collection>,
}
pub fn path() -> PathBuf {
    directories::ProjectDirs::from("", "", "mopytui")
        .expect("user config directory")
        .config_dir()
        .join("radio-collections.json")
}
pub fn read(path: &Path) -> Result<Vec<Collection>> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    ensure!(
        bytes.len() <= 8 * 1024 * 1024,
        "Collections file is too large"
    );
    let file: File = serde_json::from_slice(&bytes)
        .context("Cannot read radio collections; original file preserved")?;
    ensure!(file.version == 1, "Unsupported collections version");
    for c in &file.collections {
        valid_name(&c.name)?;
        c.config(&Config::default())?;
        ensure!(
            c.entries
                .iter()
                .all(|e| e.station.stream_url().starts_with("http://")
                    || e.station.stream_url().starts_with("https://")),
            "Invalid station URL in collection"
        );
    }
    Ok(file.collections)
}
pub fn valid_name(name: &str) -> Result<()> {
    ensure!(
        !name.trim().is_empty()
            && name.chars().count() <= 80
            && !name.chars().any(char::is_control),
        "Name must contain 1–80 printable characters"
    );
    Ok(())
}
pub fn write(path: &Path, collections: &[Collection]) -> Result<()> {
    let parent = path.parent().context("Missing collections directory")?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".radio-collections-{}-{}.tmp",
        std::process::id(),
        rand::random::<u64>()
    ));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&tmp)?;
        f.write_all(&serde_json::to_vec_pretty(&File {
            version: 1,
            collections: collections.to_vec(),
        })?)?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
#[derive(Default)]
pub struct Browser {
    pub items: Vec<Collection>,
    pub cursor: usize,
    pub visible: bool,
    pub input: Option<String>,
    pub rename: bool,
    pub delete_confirm: bool,
    pub update_confirm: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_preserves_order_disabled_stations_and_timing_not_proxy() {
        let path = std::env::temp_dir().join(format!("collections-{}.json", rand::random::<u64>()));
        let cfg = Config {
            proxy: "socks5h://secret:password@localhost:1080".into(),
            rotation_min: 2,
            rotation_max: 3,
            fade_in: 0.4,
            ..Default::default()
        };
        let entries: Vec<_> = [true, false]
            .into_iter()
            .enumerate()
            .map(|(i, enabled)| Entry {
                enabled,
                station: super::super::search::Station {
                    stationuuid: i.to_string(),
                    name: format!("Station {i}"),
                    url: format!("https://example.test/{i}"),
                    ..Default::default()
                },
            })
            .collect();
        write(
            &path,
            &[Collection::capture("Ночной ambient".into(), &entries, &cfg)],
        )
        .unwrap();
        let loaded = read(&path).unwrap();
        assert_eq!(loaded[0].name, "Ночной ambient");
        assert!(!loaded[0].entries[1].enabled);
        assert_eq!(loaded[0].entries[1].station.stationuuid, "1");
        let restored = loaded[0].config(&Config::default()).unwrap();
        assert_eq!(restored.fade_in, 0.4);
        assert_eq!(restored.rotation_min, 2);
        assert!(!fs::read_to_string(&path).unwrap().contains("password"));
        fs::write(&path, "not json").unwrap();
        assert!(read(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "not json");
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn unsafe_or_empty_names_are_rejected() {
        for name in ["", "  ", "bad\nname"] {
            assert!(valid_name(name).is_err());
        }
        assert!(valid_name("../ambient").is_ok()); // names never become file paths
    }
}
