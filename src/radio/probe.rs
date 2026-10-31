use anyhow::{Result, bail};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, ChildStdin, Command},
};

pub const BACKEND: &str = include_str!("gst_backend.py");

pub fn spawn(
    python: &str,
    url: &str,
    mode: &str,
    proxy: Option<&str>,
) -> Result<(Child, ChildStdin, tokio::process::ChildStdout)> {
    spawn_buffered(python, url, mode, proxy, 0.)
}

pub fn spawn_buffered(python: &str, url: &str, mode: &str, proxy: Option<&str>, buffer_seconds: f64)
    -> Result<(Child, ChildStdin, tokio::process::ChildStdout)> {
    let mut command = Command::new(python);
    for name in ["MOPYTUI_RADIO_PROXY", "http_proxy", "https_proxy", "all_proxy", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"] {
        command.env_remove(name);
    }
    command.env("MOPYTUI_BUFFER_SECONDS", buffer_seconds.to_string());
    if let Some(proxy) = proxy {
        command.env("MOPYTUI_RADIO_PROXY", proxy);
    }
    let mut child = command
        .args(["-u", "-c", BACKEND, url, mode])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(if mode == "play" {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .kill_on_drop(true)
        .spawn()?;
    let input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    Ok((child, input, output))
}

pub async fn probe_with_config(cfg: &super::Config, url: &str) -> Result<()> {
    let mut errors = Vec::new();
    for proxy in cfg.routes() {
        match probe_with_proxy(&cfg.python, url, Duration::from_secs_f64(cfg.station_probe_timeout), proxy.as_deref()).await {
            Ok(()) => return Ok(()),
            Err(e) => errors.push(format!("{}: {e}", super::proxy_description(proxy.as_deref()))),
        }
    }
    bail!("{}", errors.join("; "))
}

/// Success requires a raw decoded audio buffer, not HTTP/metadata/state-change.
pub async fn probe(python: &str, url: &str, timeout: Duration) -> Result<()> {
    let proxy = super::Config::default().effective_proxy();
    probe_with_proxy(python, url, timeout, proxy.as_deref()).await
}

pub async fn probe_with_proxy(
    python: &str,
    url: &str,
    timeout: Duration,
    proxy: Option<&str>,
) -> Result<()> {
    let (mut child, _input, output) = spawn(python, url, "probe", proxy)?;
    let result = tokio::time::timeout(timeout, async {
        let mut lines = BufReader::new(output).lines();
        while let Some(line) = lines.next_line().await? {
            let event: serde_json::Value = serde_json::from_str(&line)?;
            if event["event"] == "audio" {
                return Ok(());
            }
            if event["event"] == "error" {
                bail!("{}", event["message"]);
            }
        }
        bail!("Audio backend exited: check Python GI and GStreamer plugins")
    })
    .await;
    let _ = child.kill().await;
    let _ = child.wait().await;
    result.map_err(|_| anyhow::anyhow!("No decoded audio before probe timeout"))?
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[tokio::test]
    async fn configured_proxy_reaches_audio_child() {
        let path =
            std::env::temp_dir().join(format!("mopytui-proxy-decoder-{}-{:?}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::write(&path, "#!/bin/sh\n[ \"$MOPYTUI_RADIO_PROXY\" = \"socks5h://127.0.0.1:1080\" ] || exit 1\nprintf '%s\\n' '{\"event\":\"audio\"}'\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        probe_with_proxy(
            path.to_str().unwrap(),
            "ignored",
            Duration::from_secs(2),
            Some("socks5h://127.0.0.1:1080"),
        )
        .await
        .unwrap();
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn failed_direct_probe_falls_back_to_proxy_and_direct_ignores_environment() {
        let root = std::env::temp_dir().join(format!("mopytui-probe-routes-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        let script = root.join("decoder");
        std::fs::write(&script, r#"#!/usr/bin/python3
import os, json
from pathlib import Path
route = os.environ.get('MOPYTUI_RADIO_PROXY', '')
with open(Path(__file__).with_name('calls'), 'a') as f: f.write(('proxy' if route else 'direct')+'\n')
assert not any(os.environ.get(k) for k in ('HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','http_proxy','https_proxy','all_proxy'))
print(json.dumps({'event':'audio'} if route else {'event':'error','message':'direct unavailable'}), flush=True)
"#).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cfg = super::super::Config { python: script.to_string_lossy().into(), proxy: "socks5h://localhost:1080".into(), preferred_network: super::super::Network::Direct, ..Default::default() };
        probe_with_config(&cfg, "https://example.test/live").await.unwrap();
        assert_eq!(std::fs::read_to_string(root.join("calls")).unwrap(), "direct\nproxy\n");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn hung_decoder_is_killed_at_deadline() {
        let path =
            std::env::temp_dir().join(format!("mopytui-hung-decoder-{}-{:?}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::write(&path, "#!/bin/sh\nexec sleep 60\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let before = std::time::Instant::now();
        let result = probe(
            path.to_str().unwrap(),
            "ignored",
            Duration::from_millis(100),
        )
        .await;
        let message = result.unwrap_err().to_string();
        assert!(
            message.contains("timeout"),
            "unexpected probe error: {message}"
        );
        assert!(before.elapsed() < Duration::from_secs(2));
        std::fs::remove_file(path).unwrap();
    }
}
