use std::collections::HashMap;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::net::SocketAddrV6;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::bail;
use backon::ExponentialBuilder;
use backon::Retryable;
use futures::FutureExt;
use futures::StreamExt;
use futures::future::{AbortHandle, Abortable, BoxFuture};
use futures::stream::BoxStream;
use futures::stream::FuturesUnordered;
use tracing::Instrument;
use tracing::debug;
use tracing::debug_span;
use tracing::trace;
use tracing::trace_span;
use url::Url;

use crate::tracker_comms_http;
use crate::tracker_set::{TrackerSet, TrackerState, short_error};
use crate::tracker_comms_udp;
use crate::tracker_comms_udp::UdpTrackerClient;
use librqbit_core::hash_id::Id20;

pub struct TrackerComms {
    info_hash: Id20,
    peer_id: Id20,
    stats: Box<dyn TorrentStatsProvider>,
    force_tracker_interval: Option<Duration>,
    tx: Sender,
    // This MUST be set as trackers don't work with 0 port.
    announce_port: u16,
    reqwest_client: reqwest::Client,
    key: u32,
    // rqbit-patch: the live, editable tracker list (statuses are written back here).
    set: Arc<TrackerSet>,
}

#[derive(Default)]
pub enum TrackerCommsStatsState {
    #[default]
    None,
    Initializing,
    Paused,
    Live,
}

#[derive(Default)]
pub struct TrackerCommsStats {
    pub uploaded_bytes: u64,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub torrent_state: TrackerCommsStatsState,
}

impl TrackerCommsStats {
    pub fn get_left_to_download_bytes(&self) -> u64 {
        let total = self.total_bytes;
        let down = self.downloaded_bytes;
        if total >= down {
            return total - down;
        }
        0
    }

    pub fn is_completed(&self) -> bool {
        self.downloaded_bytes >= self.total_bytes
    }
}

pub trait TorrentStatsProvider: Send + Sync {
    fn get(&self) -> TrackerCommsStats;
}

impl TorrentStatsProvider for () {
    fn get(&self) -> TrackerCommsStats {
        Default::default()
    }
}

type Sender = tokio::sync::mpsc::Sender<SocketAddr>;

#[derive(Clone, Copy, Debug)]
enum UdpTrackerResolveResult {
    One(SocketAddr),
    Two(SocketAddrV4, SocketAddrV6),
}

async fn udp_tracker_to_socket_addrs(
    host: url::Host<&str>,
    port: u16,
) -> anyhow::Result<UdpTrackerResolveResult> {
    let res = match host {
        url::Host::Domain(name) => {
            // Use the first IPv4 and the first IPv6 addresses only.

            let mut v4: Option<SocketAddrV4> = None;
            let mut v6: Option<SocketAddrV6> = None;
            for addr in tokio::net::lookup_host((name, port))
                .await
                .with_context(|| format!("error looking up hostname {name}"))?
            {
                match (v4, v6, addr) {
                    (None, _, SocketAddr::V4(addr)) => v4 = Some(addr),
                    (_, None, SocketAddr::V6(addr)) => v6 = Some(addr),
                    _ => continue,
                }
            }
            let res = match (v4, v6) {
                (Some(v4), Some(v6)) => UdpTrackerResolveResult::Two(v4, v6),
                (Some(v4), None) => UdpTrackerResolveResult::One(v4.into()),
                (None, Some(v6)) => UdpTrackerResolveResult::One(v6.into()),
                _ => anyhow::bail!("zero addresses returned looking up {name}"),
            };
            trace!(?res, "resolved");
            res
        }
        url::Host::Ipv4(addr) => UdpTrackerResolveResult::One((addr, port).into()),
        url::Host::Ipv6(addr) => UdpTrackerResolveResult::One((addr, port).into()),
    };
    Ok(res)
}

impl TrackerComms {
    // TODO: fix too many args
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        info_hash: Id20,
        peer_id: Id20,
        trackers: Arc<TrackerSet>,
        stats: Box<dyn TorrentStatsProvider>,
        force_interval: Option<Duration>,
        announce_port: u16,
        reqwest_client: reqwest::Client,
        udp_client: UdpTrackerClient,
    ) -> Option<BoxStream<'static, SocketAddr>> {
        // rqbit-patch: the stream lives as long as the torrent; trackers added later start
        // announcing at once and removed ones stop, so an empty list is fine too.
        tracing::trace!(?trackers);

        let (tx, mut rx) = tokio::sync::mpsc::channel::<SocketAddr>(16);

        let s = async_stream::stream! {
            use futures::StreamExt;
            let mut changes = trackers.subscribe_changes();
            let comms = Arc::new(Self {
                info_hash,
                peer_id,
                stats,
                force_tracker_interval: force_interval,
                tx,
                announce_port,
                reqwest_client,
                key: rand::random(),
                set: trackers,
            });
            let mut running: HashMap<Url, AbortHandle> = HashMap::new();
            let mut futures = FuturesUnordered::new();
            loop {
                // Start a loop for every new tracker, stop the loops of removed ones.
                let want = comms.set.urls();
                running.retain(|url, handle| {
                    let keep = want.contains(url);
                    if !keep {
                        handle.abort();
                    }
                    keep
                });
                for url in want {
                    if running.contains_key(&url) {
                        continue;
                    }
                    let Some(fut) = comms.clone().tracker_task(url.clone(), &udp_client) else { continue };
                    let (handle, reg) = AbortHandle::new_pair();
                    let u = url.clone();
                    futures.push(Abortable::new(fut, reg).map(move |r| (u, r)));
                    running.insert(url, handle);
                }
                tokio::select! {
                    addr = rx.recv() => {
                        if let Some(addr) = addr {
                            yield addr;
                        }
                    }
                    e = futures.next(), if !futures.is_empty() => {
                        if let Some((url, Ok(Err(e)))) = e {
                            debug!(%url, "tracker loop stopped: {e:#}");
                            comms.set.update(&url, |t| {
                                t.state = TrackerState::NotWorking;
                                t.message = Some(short_error(&e));
                            });
                        }
                    }
                    r = changes.changed() => {
                        if r.is_err() {
                            // The list is gone with its torrent; keep serving what is running.
                            futures::future::pending::<()>().await;
                        }
                    }
                }
            }
        };

        Some(s.boxed())
    }

    /// The announce loop for one tracker, or None for schemes we can't talk to.
    fn tracker_task(
        self: Arc<Self>,
        url: Url,
        client: &UdpTrackerClient,
    ) -> Option<BoxFuture<'static, anyhow::Result<()>>> {
        let info_hash = self.info_hash;
        match url.scheme() {
            "udp" => {
                let span = debug_span!(parent: None, "udp_tracker", tracker = %url, info_hash = ?info_hash);
                let client = client.clone();
                Some(async move { self.task_single_tracker_monitor_udp(url, client).await }.instrument(span).boxed())
            }
            "http" | "https" => {
                let span = debug_span!(
                    parent: None,
                    "http_tracker",
                    tracker = %url,
                    info_hash = ?info_hash
                );
                Some(async move { self.task_single_tracker_monitor_http(url).await }.instrument(span).boxed())
            }
            _ => {
                debug!("unsupported tracker URL: {}", url);
                None
            }
        }
    }

    /// Sleep until the next announce, or until someone asks for a reannounce.
    async fn wait_next(&self, url: &Url, after: Duration, reannounce: &mut tokio::sync::watch::Receiver<u64>) {
        self.set.set_next(url, after);
        tokio::select! {
            _ = tokio::time::sleep(after) => {}
            r = reannounce.changed() => {
                if r.is_err() {
                    tokio::time::sleep(after).await;
                }
            }
        }
    }

    async fn task_single_tracker_monitor_http(&self, tracker_url: Url) -> anyhow::Result<()> {
        trace!(url=%tracker_url, "starting monitor");
        let mut event = Some(tracker_comms_http::TrackerRequestEvent::Started);
        let mut reannounce = self.set.subscribe_reannounce();
        reannounce.mark_unchanged();

        loop {
            self.set.update(&tracker_url, |t| t.state = TrackerState::Updating);
            let interval = (|| self.tracker_one_request_http(&tracker_url, event))
                .retry(
                    ExponentialBuilder::new()
                        .without_max_times()
                        .with_jitter()
                        .with_factor(2.)
                        .with_min_delay(Duration::from_secs(10))
                        .with_max_delay(Duration::from_secs(600)),
                )
                .notify(|err, retry_in| {
                    debug!(?retry_in, "error calling tracker: {err:#}");
                    let message = short_error(err);
                    self.set.update(&tracker_url, |t| {
                        t.state = TrackerState::NotWorking;
                        t.message = Some(message);
                        t.last_announce = Some(std::time::SystemTime::now());
                        t.next_announce = std::time::SystemTime::now().checked_add(retry_in);
                    });
                })
                .await
                .context("this shouldn't fail")?;

            event = None;
            let interval = self.force_tracker_interval.unwrap_or(interval);
            debug!("sleeping for {:?} after calling tracker", interval);
            self.wait_next(&tracker_url, interval, &mut reannounce).await;
        }
    }

    async fn tracker_one_request_http(
        &self,
        tracker_url: &Url,
        event: Option<tracker_comms_http::TrackerRequestEvent>,
    ) -> anyhow::Result<Duration> {
        let stats = self.stats.get();
        let request = tracker_comms_http::TrackerRequest {
            info_hash: &self.info_hash,
            peer_id: &self.peer_id,
            port: self.announce_port,
            uploaded: stats.uploaded_bytes,
            downloaded: stats.downloaded_bytes,
            left: stats.get_left_to_download_bytes(),
            compact: true,
            no_peer_id: false,
            event,
            ip: None,
            numwant: None,
            key: Some(self.key),
            trackerid: None,
        };

        let mut url = tracker_url.clone();

        let mut queries = request.as_querystring();
        if let Some(url_query) = url.query() {
            queries.push_str(&format!("&{}", url_query));
        }
        url.set_query(Some(&queries));

        let response: reqwest::Response = self.reqwest_client.get(url).send().await?;
        if !response.status().is_success() {
            anyhow::bail!("tracker responded with {:?}", response.status());
        }
        let bytes = response.bytes().await?;
        if let Ok((error, _)) =
            bencode::from_bytes_with_rest::<tracker_comms_http::TrackerError>(&bytes)
        {
            anyhow::bail!(
                "tracker returned failure. Failure reason: {}",
                error.failure_reason
            )
        };
        let response = bencode::from_bytes_with_rest::<tracker_comms_http::TrackerResponse>(&bytes)
            .map_err(|e| {
                tracing::trace!("error deserializing TrackerResponse: {e:#}");
                e.into_kind()
            })?
            .0;

        let mut n = 0;
        for peer in response.iter_peers() {
            self.tx.send(peer).await?;
            n += 1;
        }
        let warning = response.warning_message.as_ref().map(|w| String::from_utf8_lossy(w.as_ref()).into_owned());
        self.set.update(tracker_url, |t| {
            t.state = TrackerState::Working;
            t.seeders = Some(response.complete);
            t.leechers = Some(response.incomplete);
            t.peers_received += n;
            t.last_announce = Some(std::time::SystemTime::now());
            t.message = warning;
        });
        Ok(Duration::from_secs(
            response.min_interval.unwrap_or(response.interval),
        ))
    }

    async fn task_single_tracker_monitor_udp(
        &self,
        url: Url,
        client: UdpTrackerClient,
    ) -> anyhow::Result<()> {
        if url.scheme() != "udp" {
            bail!("expected UDP scheme in {}", url);
        }
        let (host, port) = (
            url.host().context("missing host")?,
            url.port().context("missing port")?,
        );

        let mut sleep_interval: Option<Duration> = None;
        let mut prev_addrs: Option<UdpTrackerResolveResult> = None;
        let mut reannounce = self.set.subscribe_reannounce();
        reannounce.mark_unchanged();
        loop {
            if let Some(i) = sleep_interval {
                trace!(interval=?sleep_interval, "sleeping");
                self.wait_next(&url, i, &mut reannounce).await;
            }
            self.set.update(&url, |t| t.state = TrackerState::Updating);

            // This should retry forever until the addrs are resolved.
            let addrs = (async || {
                udp_tracker_to_socket_addrs(host.clone(), port)
                    .instrument(trace_span!("resolve", ?host))
                    .await
                    .or_else(|err| prev_addrs.ok_or(err))
            })
            .retry(
                ExponentialBuilder::new()
                    .without_max_times()
                    .with_max_delay(Duration::from_secs(60))
                    .with_jitter(),
            )
            .notify(|err, retry| {
                debug!(retry_in=?retry, "error resolving tracker: {err:#}");
                let message = short_error(err);
                self.set.update(&url, |t| {
                    t.state = TrackerState::NotWorking;
                    t.message = Some(message);
                });
            })
            .await
            .context("this shouldn't happen: failed resolving tracker addrs")?;

            prev_addrs = Some(addrs);

            match addrs {
                UdpTrackerResolveResult::One(addr) => {
                    match self
                        .tracker_one_request_udp(&url, addr, &client)
                        .instrument(trace_span!("udp request", ?addr))
                        .await
                    {
                        Ok(sleep) => sleep_interval = Some(sleep),
                        Err(_) => {
                            sleep_interval = Some(sleep_interval.unwrap_or(Duration::from_secs(60)))
                        }
                    }
                }
                UdpTrackerResolveResult::Two(v4, v6) => {
                    let (r4, r6) = tokio::join!(
                        self.tracker_one_request_udp(&url, v4.into(), &client)
                            .instrument(trace_span!("udp request", addr=?v4)),
                        self.tracker_one_request_udp(&url, v6.into(), &client)
                            .instrument(trace_span!("udp request", addr=?v6))
                    );
                    sleep_interval = Some(
                        r4.or(r6)
                            .ok()
                            .or(sleep_interval)
                            .unwrap_or(Duration::from_secs(60)),
                    )
                }
            }
        }
    }

    async fn tracker_one_request_udp(
        &self,
        url: &Url,
        addr: SocketAddr,
        client: &UdpTrackerClient,
    ) -> anyhow::Result<Duration> {
        use tracker_comms_udp::*;

        let stats = self.stats.get();
        let request = AnnounceFields {
            info_hash: self.info_hash,
            peer_id: self.peer_id,
            downloaded: stats.downloaded_bytes,
            left: stats.get_left_to_download_bytes(),
            uploaded: stats.uploaded_bytes,
            event: match stats.torrent_state {
                TrackerCommsStatsState::None => EVENT_NONE,
                TrackerCommsStatsState::Initializing => EVENT_STARTED,
                TrackerCommsStatsState::Paused => EVENT_STOPPED,
                TrackerCommsStatsState::Live => {
                    if stats.is_completed() {
                        EVENT_COMPLETED
                    } else {
                        EVENT_STARTED
                    }
                }
            },
            key: self.key,
            port: self.announce_port,
        };

        match client.announce(addr, request).await {
            Ok(response) => {
                trace!(len = response.addrs.len(), "received announce response");
                let n = response.addrs.len() as u64;
                self.set.update(url, |t| {
                    t.state = TrackerState::Working;
                    t.seeders = Some(response.seeders as u64);
                    t.leechers = Some(response.leechers as u64);
                    t.peers_received += n;
                    t.last_announce = Some(std::time::SystemTime::now());
                    t.message = None;
                });
                for addr in response.addrs {
                    self.tx.send(addr).await.context("rx closed")?;
                }
                let sleep = response.interval.max(5);
                let sleep = Duration::from_secs(sleep as u64);
                Ok(sleep)
            }
            Err(e) => {
                debug!(?addr, "error reading announce response: {e:#}");
                let message = short_error(&e);
                self.set.update(url, |t| {
                    t.state = TrackerState::NotWorking;
                    t.message = Some(message);
                    t.last_announce = Some(std::time::SystemTime::now());
                });
                Err(e)
            }
        }
    }
}
