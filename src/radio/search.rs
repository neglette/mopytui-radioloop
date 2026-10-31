use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, time::Duration};
#[cfg(test)]
use std::error::Error;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Station {
    pub stationuuid: String,
    pub name: String,
    pub url_resolved: String,
    pub url: String,
    pub codec: String,
    pub bitrate: u32,
    pub tags: String,
}
impl Station {
    pub fn stream_url(&self) -> &str {
        if self.url_resolved.is_empty() {
            &self.url
        } else {
            &self.url_resolved
        }
    }
    pub fn id(&self) -> &str {
        if self.stationuuid.is_empty() {
            self.stream_url()
        } else {
            &self.stationuuid
        }
    }
}

/// Search name OR tags, merge by stable UUID, and fail over between mirrors.
pub async fn search(query: &str, servers: &[String]) -> Result<Vec<Station>> {
    search_with_config(query, servers, &super::Config::default()).await
}

pub async fn search_with_config(query: &str, servers: &[String], cfg: &super::Config) -> Result<Vec<Station>> {
    search_with_progress(query, servers, cfg, |_| {}).await
}

/// SRV is the documented alternative to A/AAAA + reverse DNS. Give each
/// stage its own deadline: one slow PTR must not discard other valid names.
async fn discover_servers() -> Vec<String> {
    let Ok(builder) = hickory_resolver::TokioResolver::builder_tokio() else { return vec![]; };
    let resolver = builder.build();
    let mut names = Vec::new();
    if let Ok(Ok(records)) = tokio::time::timeout(Duration::from_secs(1), resolver.srv_lookup("_api._tcp.radio-browser.info.")).await {
        for record in records.iter() {
            if let Some(url) = mirror_url(&record.target().to_utf8()) { names.push(url); }
        }
    }
    if names.is_empty() {
        if let Ok(Ok(addresses)) = tokio::time::timeout(Duration::from_secs(1), resolver.lookup_ip("all.api.radio-browser.info.")).await {
            let lookups = addresses.iter().take(32).map(|address| {
                let resolver = &resolver;
                async move {
                    match tokio::time::timeout(Duration::from_secs(1), resolver.reverse_lookup(address)).await {
                        Ok(Ok(records)) => records.iter().filter_map(|record| mirror_url(&record.to_utf8())).collect::<Vec<_>>(),
                        _ => vec![],
                    }
                }
            });
            for found in futures::future::join_all(lookups).await { names.extend(found); }
        }
    }
    names
}

/// A successful discovery replaces the bootstrap addresses. Never prepend
/// de1 (or any configured server) to the discovered, randomized list.
fn server_order(discovered: Vec<String>, configured: &[String], rng: &mut impl rand::Rng) -> Vec<String> {
    use rand::seq::SliceRandom;
    let mut mirrors = normalize_servers(discovered);
    if mirrors.is_empty() { mirrors = normalize_servers(configured.iter().cloned()); }
    mirrors.shuffle(rng);
    mirrors
}
fn normalize_servers(servers: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    servers.into_iter().filter_map(|server| {
        let url = reqwest::Url::parse(server.trim()).ok()?;
        if url.scheme() != "https" || url.host_str().is_none() || url.query().is_some() || url.fragment().is_some() { return None; }
        let value = url.to_string().trim_end_matches('/').to_owned();
        seen.insert(value.clone()).then_some(value)
    }).collect()
}

fn mirror_url(name: &str) -> Option<String> {
    let host = name.trim_end_matches('.').to_ascii_lowercase();
    (host.ends_with(".api.radio-browser.info") && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.'))
        .then(|| format!("https://{host}"))
}

pub async fn search_with_progress(
    query: &str, servers: &[String], cfg: &super::Config,
    progress: impl Fn(String),
) -> Result<Vec<Station>> {
    if query.trim().is_empty() { return Ok(Vec::new()); }
    progress("Searching: discovering Radio Browser servers…".into());
    let discovered = discover_servers().await;
    let fallback = discovered.is_empty();
    let mirrors = server_order(discovered, servers, &mut rand::rng());
    progress(format!("Searching: {} servers · {}", mirrors.len(), if fallback { "DNS unavailable; configured fallback" } else { "DNS discovery; shuffled order" }));
    let mut errors = Vec::new();
    for proxy in cfg.routes() {
        let route = super::proxy_description(proxy.as_deref());
        // Reserve a separate budget for each route, so a broken direct
        // connection cannot consume all time before trying the user's proxy.
        let result = tokio::time::timeout(Duration::from_secs(12),
            search_servers(query, &mirrors, proxy.as_deref(), &progress)).await;
        match result {
            Ok(Ok(stations)) => return Ok(stations),
            Ok(Err(e)) => errors.push(format!("{route}: {e}")),
            Err(_) => errors.push(format!("{route}: 12 s search timeout")),
        }
    }
    bail!("Radio Browser unavailable — {}", errors.join("; "))
}

pub async fn search_with_proxy(query: &str, servers: &[String], proxy: Option<&str>) -> Result<Vec<Station>> {
    tokio::time::timeout(Duration::from_secs(12), search_servers(query, servers, proxy, &|_| {}))
        .await.map_err(|_| anyhow::anyhow!("Radio Browser: 12 s search timeout"))?
}

async fn search_servers(query: &str, servers: &[String], proxy: Option<&str>, progress: &impl Fn(String)) -> Result<Vec<Station>> {
    search_servers_using(query, servers, proxy, progress, |query, server, proxy| async move {
        search_server(&query, &server, proxy.as_deref()).await
    }).await
}
async fn search_servers_using<F, Fut>(query: &str, servers: &[String], proxy: Option<&str>, progress: &impl Fn(String), fetch: F) -> Result<Vec<Station>>
where F: Fn(String, String, Option<String>) -> Fut, Fut: std::future::Future<Output = Result<Vec<Station>>> {
    if query.trim().is_empty() { return Ok(Vec::new()); }
    let mut last_error = "No servers configured".to_owned();
    for (index, server) in servers.iter().enumerate() {
        let host = reqwest::Url::parse(server)?.host_str().unwrap_or("server").to_owned();
        progress(format!("Searching: {} · {} ({}/{})", super::proxy_description(proxy), host, index+1, servers.len()));
        match fetch(query.to_owned(), server.clone(), proxy.map(str::to_owned)).await {
            Ok(stations) => return Ok(stations),
            Err(e) => {
                last_error = format!("{host}: {e}");
                progress(format!("{last_error} · trying next server/route…"));
            }
        }
    }
    bail!("{last_error}")
}

async fn search_server(query: &str, server: &str, proxy: Option<&str>) -> Result<Vec<Station>> {
    let make_url = |field: &str| -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&format!("{}/json/stations/search", server.trim_end_matches('/')))?;
        url.query_pairs_mut().extend_pairs([(field, query), ("hidebroken", "true"), ("limit", "100"), ("order", "votes"), ("reverse", "true")]);
        Ok(url)
    };
    let name = make_url("name")?;
    let tags = make_url("tagList")?;
    // Name and tag searches are independent; don't make their timeouts additive.
    let (names, tags) = tokio::join!(curl_search_with_proxy(&name, proxy), curl_search_with_proxy(&tags, proxy));
    merge_replies([names, tags])
}
fn merge_replies(replies: [Result<Vec<Station>>; 2]) -> Result<Vec<Station>> {
    let mut seen = HashSet::new(); let mut merged = Vec::new();
    let mut success = false; let mut errors = Vec::new();
    for reply in replies {
        match reply {
            Ok(stations) => {
                success = true;
                for mut station in stations {
                    station.name.retain(|c| !c.is_control()); station.codec.retain(|c| !c.is_control());
                    if (station.stream_url().starts_with("https://") || station.stream_url().starts_with("http://")) && seen.insert(station.id().to_owned()) { merged.push(station); }
                }
            }
            Err(e) => errors.push(e.to_string()),
        }
    }
    if success { Ok(merged) } else { errors.dedup(); bail!("{}", errors.join("; ")) }
}

/// Optional transport fallback uses the user's working system curl / CA store.
/// No shell, no certificate bypass, no station playback changes.
#[cfg(test)]
async fn curl_search(url: &reqwest::Url) -> Result<Vec<Station>> {
    curl_search_with_proxy(url, None).await
}

fn curl_command(proxy: Option<&str>) -> tokio::process::Command {
    let mut command = tokio::process::Command::new("curl");
    command.arg("-q");
    if let Some(proxy) = proxy {
        command.args(["--proxy", proxy, "--noproxy", ""]);
    } else {
        command.args(["--proxy", "", "--noproxy", "*"]);
    }
    command
}

async fn curl_search_with_proxy(url: &reqwest::Url, proxy: Option<&str>) -> Result<Vec<Station>> {
    let child = curl_command(proxy)
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "5",
            "--max-time",
            "5",
            "--max-filesize",
            "4194304",
            "--user-agent",
            concat!("mopytui-radio/", env!("CARGO_PKG_VERSION")),
            "--url",
            url.as_str(),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| anyhow::anyhow!("curl fallback unavailable: {e}"))?;
    let output = tokio::time::timeout(Duration::from_secs(6), child.wait_with_output())
        .await
        .map_err(|_| anyhow::anyhow!("curl fallback timed out"))??;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        bail!(
            "curl: {}",
            message.trim().chars().take(400).collect::<String>()
        );
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

/// Reqwest's Display omits the underlying DNS/TLS/socket failure. Put the
/// actual cause first and omit the long query URL so it fits in the TUI.
#[cfg(test)]
fn network_error(error: &reqwest::Error) -> String {
    let mut causes = Vec::new();
    let mut source = error.source();
    while let Some(cause) = source {
        causes.push(cause.to_string());
        source = cause.source();
    }
    causes.reverse();
    causes.dedup();
    let host = error
        .url()
        .and_then(|url| url.host_str())
        .unwrap_or("server");
    let kind = if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connection failed"
    } else if error.is_decode() {
        "invalid API response"
    } else {
        "request failed"
    };
    let reason = if causes.is_empty() {
        error
            .status()
            .map(|s| s.to_string())
            .unwrap_or_else(|| kind.to_owned())
    } else {
        causes.join("; ")
    };
    format!("{reason} [{host}: {kind}]")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires access to Radio Browser and system curl"]
    async fn system_curl_live_search() {
        let url = reqwest::Url::parse(
            "https://de1.api.radio-browser.info/json/stations/search?name=ambient&limit=1",
        )
        .unwrap();
        let stations = curl_search(&url).await.unwrap();
        assert!(!stations.is_empty());
        assert!(!stations[0].stream_url().is_empty());
    }

    #[tokio::test]
    async fn diagnostic_preserves_root_cause_without_query_url() {
        // Port zero cannot host a server; disabling proxies makes the test
        // deterministic and independent of DNS and external services.
        let error = reqwest::Client::builder()
            .no_proxy()
            .no_hickory_dns()
            .timeout(Duration::from_secs(1))
            .build()
            .unwrap()
            .get("http://127.0.0.1:0/json/stations/search?name=ambient")
            .send()
            .await
            .unwrap_err();
        let message = network_error(&error);
        assert!(message.contains("127.0.0.1"));
        assert!(!message.contains("name=ambient"));
        let mut root: &(dyn Error + 'static) = &error;
        while let Some(next) = root.source() {
            root = next;
        }
        assert!(message.starts_with(&root.to_string()), "{message}");
    }
}

#[cfg(test)]
mod proxy_command_tests {
    #[test]
    fn explicit_proxy_overrides_bypass_and_ignores_curlrc() {
        let command = super::curl_command(Some("socks5h://127.0.0.1:1080"));
        let args: Vec<_> = command
            .as_std()
            .get_args()
            .map(|x| x.to_str().unwrap())
            .collect();
        assert_eq!(
            args,
            ["-q", "--proxy", "socks5h://127.0.0.1:1080", "--noproxy", ""]
        );
        let command = super::curl_command(None);
        assert_eq!(command.as_std().get_args().count(), 5);
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[tokio::test]
    async fn http_503_moves_to_next_mirror_and_reports_progress() {
        let updates = std::sync::Mutex::new(Vec::new());
        let calls = std::sync::Mutex::new(Vec::new());
        let result = search_servers_using("ambient", &["https://first.test".into(), "https://second.test".into()], Some("socks5h://localhost:1080"), &|s| updates.lock().unwrap().push(s), |query, server, proxy| {
            assert_eq!(query, "ambient");
            assert_eq!(proxy.as_deref(), Some("socks5h://localhost:1080"));
            calls.lock().unwrap().push(server.clone());
            std::future::ready(if server.contains("first") { Err(anyhow::anyhow!("HTTP 503")) } else { Ok(vec![Station { name: "Found".into(), ..Default::default() }]) })
        }).await.unwrap();
        assert_eq!(calls.lock().unwrap().len(), 2);
        assert_eq!(result[0].name, "Found");
        assert!(updates.lock().unwrap().iter().any(|s| s.contains("503")));
        assert!(updates.lock().unwrap().iter().any(|s| s.contains("second.test (2/2)")));
    }
    #[test]
    fn partial_reply_is_kept_and_mirrors_are_validated() {
        let station = Station { stationuuid: "one".into(), name: "Station".into(), url: "https://audio.test/live".into(), ..Default::default() };
        assert_eq!(merge_replies([Ok(vec![station.clone()]), Err(anyhow::anyhow!("HTTP 503"))]).unwrap(), vec![station.clone()]);
        assert_eq!(merge_replies([Ok(vec![station.clone()]), Ok(vec![station])]).unwrap().len(), 1);
        assert!(merge_replies([Err(anyhow::anyhow!("HTTP 503")), Err(anyhow::anyhow!("HTTP 503"))]).unwrap_err().to_string().contains("503"));
        assert_eq!(mirror_url("DE1.api.radio-browser.info."), Some("https://de1.api.radio-browser.info".into()));
        assert!(mirror_url("evil.test").is_none());
        assert!(mirror_url("other.test/path.api.radio-browser.info").is_none());
    }
}

#[cfg(test)]
mod discovery_order_tests {
    use super::*;
    use rand::SeedableRng;
    #[test]
    fn discovered_servers_replace_bootstrap_and_are_shuffled() {
        let discovered = vec!["https://a.api.radio-browser.info".into(), "https://b.api.radio-browser.info".into(), "https://c.api.radio-browser.info".into()];
        let configured = vec!["https://de1.api.radio-browser.info".into()];
        let mut first = HashSet::new();
        for seed in 0..16 {
            let actual = server_order(discovered.clone(), &configured, &mut rand::rngs::StdRng::seed_from_u64(seed));
            assert_eq!(actual.len(), 3);
            assert!(!actual.contains(&configured[0]));
            for server in &discovered { assert!(actual.contains(server)); }
            first.insert(actual[0].clone());
        }
        assert!(first.len() > 1, "first server must not be permanently pinned");
    }
    #[test]
    fn fallback_is_used_only_when_discovery_has_no_usable_servers() {
        let configured = vec!["https://DE1.api.radio-browser.info/".into(), "https://de1.api.radio-browser.info".into(), "not a URL".into()];
        let actual = server_order(vec![], &configured, &mut rand::rngs::StdRng::seed_from_u64(1));
        assert_eq!(actual, vec!["https://de1.api.radio-browser.info"]);
        let actual = server_order(vec!["https://de1.api.radio-browser.info/".into(), "https://de1.api.radio-browser.info".into()], &[], &mut rand::rngs::StdRng::seed_from_u64(1));
        assert_eq!(actual.len(), 1);
    }
}
