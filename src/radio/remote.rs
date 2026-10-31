//! Private control socket for the separate MPD gateway. No Mopidy RPC here.
use super::player::{Command, Control, Snapshot};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("MOPYTUI_CONTROL_SOCKET") {
        return path.into();
    }
    directories::ProjectDirs::from("", "", "mopytui")
        .map(|dirs| dirs.cache_dir().join("radio-control.sock"))
        .unwrap_or_else(|| PathBuf::from("/tmp/mopytui-radio-control.sock"))
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Reply {
    pub handled: bool,
    pub lines: Vec<String>,
    pub error: Option<String>,
}
impl Reply {
    fn ok(lines: Vec<String>) -> Self {
        Self {
            handled: true,
            lines,
            error: None,
        }
    }
    fn error(message: &str) -> Self {
        Self {
            handled: true,
            lines: vec![],
            error: Some(message.into()),
        }
    }
    pub fn forward() -> Self {
        Self {
            handled: false,
            lines: vec![],
            error: None,
        }
    }
}
fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

pub fn dispatch(line: &str, snapshot: &Snapshot) -> (Reply, Option<Command>) {
    if !snapshot.running {
        return (Reply::forward(), None);
    }
    let mut args = line.split_whitespace();
    let name = args.next().unwrap_or("");
    let argument = args.next().map(|a| a.trim_matches('"'));
    let command = match name {
        "next" => Some(Command::Next),
        "previous" => Some(Command::Previous),
        "play" => Some(Command::SetPaused(false)),
        "pause" => match argument {
            None => Some(Command::Pause),
            Some("0") => Some(Command::SetPaused(false)),
            Some("1") => Some(Command::SetPaused(true)),
            _ => return (Reply::error("pause expects 0 or 1"), None),
        },
        "stop" => Some(Command::Stop),
        // ACK but never forward these to Mopidy or to the radio pipeline.
        "seek" | "seekid" | "seekcur" => return (Reply::ok(vec![]), None),
        "setvol" | "volume" => {
            let Some(value) = argument.and_then(|v| v.parse::<i32>().ok()) else {
                return (Reply::error("expected integer volume"), None);
            };
            if name == "setvol" {
                Some(Command::SetVolume(value.clamp(0, 100) as f64 / 100.))
            } else {
                Some(Command::Volume(value as f64 / 100.))
            }
        }
        "status" => {
            let state = if snapshot.paused { "pause" } else { "play" };
            let mut lines = vec![
                format!("volume: {:.0}", snapshot.volume * 100.),
                "repeat: 0".into(),
                "random: 0".into(),
                "single: 0".into(),
                "consume: 0".into(),
                "playlist: 1".into(),
                format!("playlistlength: {}", snapshot.station_count),
                format!("state: {state}"),
                format!("elapsed: {}.000", snapshot.elapsed),
            ];
            if snapshot.current.is_some() {
                lines.push(format!("song: {}", snapshot.station_index));
                lines.push(format!("songid: {}", snapshot.station_index + 1));
            }
            return (Reply::ok(lines), None);
        }
        "currentsong" => {
            let lines = if snapshot.current.is_some() {
                vec![
                    format!("file: {}", clean(&snapshot.current_url)),
                    format!("Name: {}", clean(&snapshot.current_name)),
                    format!(
                        "Title: {}",
                        clean(if snapshot.title.is_empty() {
                            &snapshot.current_name
                        } else {
                            &snapshot.title
                        })
                    ),
                    format!("Artist: {}", clean(&snapshot.artist)),
                    format!("Album: {}", clean(&snapshot.album)),
                    "Comment: mopytui-radio".into(),
                    format!("Pos: {}", snapshot.station_index),
                    format!("Id: {}", snapshot.station_index + 1),
                ]
            } else {
                vec![]
            };
            return (Reply::ok(lines), None);
        }
        _ => return (Reply::forward(), None),
    };
    (Reply::ok(vec![]), command)
}

#[cfg(unix)]
pub struct Server {
    task: tokio::task::JoinHandle<()>,
    path: PathBuf,
}
#[cfg(unix)]
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}
#[cfg(unix)]
pub async fn serve(control: Control) -> anyhow::Result<Server> {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        net::{UnixListener, UnixStream},
    };
    let path = socket_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        anyhow::ensure!(
            metadata.file_type().is_socket(),
            "control path is not a socket"
        );
        anyhow::ensure!(
            UnixStream::connect(&path).await.is_err(),
            "another mopytui owns the control socket"
        );
        std::fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    let task = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let control = control.clone();
            tokio::spawn(async move {
                let work = async {
                    let (read, mut write) = stream.into_split();
                    let mut reader = BufReader::new(read.take(8192));
                    let mut line = String::new();
                    reader.read_line(&mut line).await?;
                    let (reply, command) = dispatch(line.trim_end(), &control.snapshot());
                    if let Some(command) = command {
                        control.send_confirmed(command).await?;
                    }
                    write
                        .write_all(format!("{}\n", serde_json::to_string(&reply)?).as_bytes())
                        .await?;
                    Ok::<_, anyhow::Error>(())
                };
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), work).await;
            });
        }
    });
    Ok(Server { task, path })
}

#[cfg(unix)]
pub async fn request(line: &str) -> anyhow::Result<Reply> {
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        net::UnixStream,
    };
    let stream = match UnixStream::connect(socket_path()).await {
        Ok(stream) => stream,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(Reply::forward());
        }
        Err(e) => return Err(e.into()),
    };
    let request = async {
        let (read, mut write) = stream.into_split();
        write.write_all(format!("{line}\n").as_bytes()).await?;
        let mut response = String::new();
        BufReader::new(read.take(65536))
            .read_line(&mut response)
            .await?;
        Ok::<_, anyhow::Error>(serde_json::from_str(&response)?)
    };
    tokio::time::timeout(std::time::Duration::from_secs(2), request).await?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn active() -> Snapshot {
        Snapshot {
            running: true,
            volume: 0.8,
            ..Default::default()
        }
    }
    #[test]
    fn current_song_exports_radio_identity_and_track_tags() {
        let mut snapshot = active();
        snapshot.current = Some("station".into());
        snapshot.current_name = "Station".into();
        snapshot.artist = "Artist".into();
        snapshot.title = "Track".into();
        let lines = dispatch("currentsong", &snapshot).0.lines;
        for expected in [
            "Name: Station",
            "Artist: Artist",
            "Title: Track",
            "Comment: mopytui-radio",
        ] {
            assert!(lines.iter().any(|l| l == expected));
        }
        snapshot.title.clear();
        snapshot.artist.clear();
        assert!(
            dispatch("currentsong", &snapshot)
                .0
                .lines
                .contains(&"Title: Station".into())
        );
    }
    #[test]
    fn seek_is_acknowledged_without_any_command_while_radio_is_active() {
        for command in ["seek 0 45", "seekid 12 30", "seekcur +10", "seekcur -10"] {
            let (reply, command) = dispatch(command, &active());
            assert!(reply.handled && reply.error.is_none() && command.is_none());
        }
        assert!(!dispatch("seekcur +10", &Snapshot::default()).0.handled);
    }
    #[test]
    fn mpc_play_pause_next_previous_and_volume_target_radio() {
        assert!(matches!(
            dispatch("play", &active()).1,
            Some(Command::SetPaused(false))
        ));
        assert!(matches!(
            dispatch("pause 1", &active()).1,
            Some(Command::SetPaused(true))
        ));
        assert!(matches!(
            dispatch("pause 0", &active()).1,
            Some(Command::SetPaused(false))
        ));
        assert!(matches!(
            dispatch("pause", &active()).1,
            Some(Command::Pause)
        ));
        assert!(matches!(dispatch("next", &active()).1, Some(Command::Next)));
        assert!(matches!(
            dispatch("previous", &active()).1,
            Some(Command::Previous)
        ));
        assert!(
            matches!(dispatch("setvol 35", &active()).1, Some(Command::SetVolume(v)) if v == 0.35)
        );
        assert!(
            matches!(dispatch("volume -5", &active()).1, Some(Command::Volume(v)) if v == -0.05)
        );
        assert!(!dispatch("play", &Snapshot::default()).0.handled);
    }
}
