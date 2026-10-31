//! Run independently of the TUI: when its private socket disappears, forward
//! to ordinary Mopidy. This keeps mpc usable after the TUI closes.
#[cfg(unix)]
mod gateway {
    use anyhow::{Context, Result, bail};
    use mopytui::radio::remote;
    use std::{net::SocketAddr, time::Duration};
    use tokio::{
        io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
        net::{TcpListener, TcpStream, UnixListener, UnixStream},
    };

    trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
    impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}
    type Socket = Box<dyn Stream>;
    struct Upstream {
        read: BufReader<tokio::io::ReadHalf<Socket>>,
        write: tokio::io::WriteHalf<Socket>,
    }
    impl Upstream {
        async fn connect(address: &str) -> Result<Self> {
            let stream: Socket = if address.starts_with('/') {
                Box::new(
                    tokio::time::timeout(Duration::from_secs(5), UnixStream::connect(address))
                        .await??,
                )
            } else {
                Box::new(
                    tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(address))
                        .await??,
                )
            };
            let (read, write) = tokio::io::split(stream);
            let mut read = BufReader::new(read);
            let mut greeting = String::new();
            tokio::time::timeout(Duration::from_secs(5), read.read_line(&mut greeting)).await??;
            anyhow::ensure!(
                greeting.starts_with("OK MPD "),
                "upstream is not an MPD server"
            );
            Ok(Self { read, write })
        }
        async fn command(&mut self, command: &str) -> Result<(Vec<String>, Option<String>)> {
            self.write
                .write_all(format!("{command}\n").as_bytes())
                .await?;
            let mut lines = vec![];
            loop {
                let mut line = String::new();
                if self.read.read_line(&mut line).await? == 0 {
                    bail!("Mopidy disconnected");
                }
                let line = line.trim_end_matches(['\r', '\n']).to_owned();
                if line == "OK" {
                    return Ok((lines, None));
                }
                if line.starts_with("ACK ") {
                    return Ok((lines, Some(line)));
                }
                lines.push(line);
                anyhow::ensure!(lines.len() < 100_000, "upstream response too large");
            }
        }
    }
    async fn execute(
        line: &str,
        address: &str,
        upstream: &mut Option<Upstream>,
    ) -> Result<(Vec<String>, Option<String>)> {
        let reply = remote::request(line).await?;
        if reply.handled {
            let error = reply.error.map(|e| {
                format!(
                    "ACK [2@0] {{{}}} {}",
                    line.split_whitespace().next().unwrap_or(""),
                    e
                )
            });
            return Ok((reply.lines, error));
        }
        if upstream.is_none() {
            *upstream = Some(Upstream::connect(address).await?);
        }
        upstream.as_mut().unwrap().command(line).await
    }
    fn acknowledgement(error: String, index: usize) -> String {
        if let Some((left, right)) = error.split_once('@') {
            if let Some((_, suffix)) = right.split_once(']') {
                return format!("{left}@{index}]{suffix}\n");
            }
        }
        format!(
            "ACK [5@{index}] {{}} {}\n",
            error.replace(['\r', '\n'], " ")
        )
    }
    async fn session(stream: Socket, address: String) -> Result<()> {
        let (read, mut write) = tokio::io::split(stream);
        let mut reader = BufReader::new(read);
        let mut upstream = None;
        write.write_all(b"OK MPD 0.23.5\n").await?;
        let mut batch: Option<(bool, Vec<String>)> = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await? == 0 {
                return Ok(());
            }
            anyhow::ensure!(line.len() <= 8192, "command too long");
            let line = line.trim_end_matches(['\r', '\n']).to_owned();
            if line == "close" {
                return Ok(());
            }
            if line == "command_list_begin" || line == "command_list_ok_begin" {
                if batch.is_some() {
                    bail!("nested command list");
                }
                batch = Some((line == "command_list_ok_begin", vec![]));
                continue;
            }
            let (list_ok, commands) = if line == "command_list_end" {
                batch.take().context("command_list_end without begin")?
            } else if let Some((_, commands)) = &mut batch {
                anyhow::ensure!(commands.len() < 1024, "command list too large");
                commands.push(line);
                continue;
            } else {
                (false, vec![line])
            };
            let mut failed = false;
            for (index, line) in commands.iter().enumerate() {
                let result = execute(line, &address, &mut upstream).await;
                let (lines, error) = match result {
                    Ok(result) => result,
                    Err(e) => {
                        upstream = None;
                        (
                            vec![],
                            Some(format!(
                                "ACK [5@0] {{{}}} {e}",
                                line.split_whitespace().next().unwrap_or("")
                            )),
                        )
                    }
                };
                for line in lines {
                    write.write_all(format!("{line}\n").as_bytes()).await?;
                }
                if let Some(error) = error {
                    write
                        .write_all(acknowledgement(error, index).as_bytes())
                        .await?;
                    failed = true;
                    break;
                }
                if list_ok {
                    write.write_all(b"list_OK\n").await?;
                }
            }
            if !failed {
                write.write_all(b"OK\n").await?;
            }
        }
    }
    pub async fn run() -> Result<()> {
        let mut listen = "127.0.0.1:6601".to_owned();
        let mut upstream = "127.0.0.1:6600".to_owned();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--listen" => listen = args.next().context("--listen requires ADDRESS:PORT")?,
                "--upstream" => {
                    upstream = args.next().context("--upstream requires ADDRESS:PORT")?
                }
                "--help" | "-h" => {
                    println!(
                        "mopytui-mpd-proxy [--listen 127.0.0.1:6601] [--upstream 127.0.0.1:6600]\nRun under the same user as mopytui. Point mpc at --listen.\nRadio active: play/pause/next/previous/volume control radio, seek is ignored.\nRadio inactive or mopytui closed: commands go to Mopidy.\nMOPYTUI_CONTROL_SOCKET overrides the private control socket path."
                    );
                    return Ok(());
                }
                _ => bail!("unknown option: {arg}"),
            }
        }
        if listen.starts_with('/') {
            use std::os::unix::fs::PermissionsExt;
            let listener = UnixListener::bind(&listen)?;
            std::fs::set_permissions(&listen, std::fs::Permissions::from_mode(0o600))?;
            loop {
                let (stream, _) = listener.accept().await?;
                let upstream = upstream.clone();
                tokio::spawn(async move {
                    let _ = session(Box::new(stream), upstream).await;
                });
            }
        }
        let bind: SocketAddr = listen
            .parse()
            .context("--listen must be an IP address and port")?;
        anyhow::ensure!(
            bind.ip().is_loopback(),
            "only loopback listeners are supported"
        );
        if let Ok(target) = upstream.parse::<SocketAddr>() {
            anyhow::ensure!(
                !(target.ip().is_loopback() && target.port() == bind.port()),
                "listen and upstream ports must differ"
            );
        }
        let listener = TcpListener::bind(bind)
            .await
            .context("cannot bind MPD gateway")?;
        eprintln!("MPD gateway listening on {listen}; Mopidy upstream {upstream}");
        loop {
            let (stream, _) = listener.accept().await?;
            let upstream = upstream.clone();
            tokio::spawn(async move {
                if let Err(e) = session(Box::new(stream), upstream).await {
                    eprintln!("MPD gateway: {e}");
                }
            });
        }
    }
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        gateway::run().await
    }
    #[cfg(not(unix))]
    {
        anyhow::bail!("MPD radio gateway currently requires a Unix system")
    }
}
