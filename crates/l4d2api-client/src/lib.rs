//! `l4d2api-client`: session client over the game's `ems/tiktools_l4d2/` files.
//!
//! One outstanding mutation at a time (single-flight). Writes are atomic
//! (temp file + rename); reads validate the full record and ignore partial
//! or malformed snapshots. Timeouts never trigger an automatic re-execute:
//! the game may already have acted.
//!
//! Cross-process safety: a lock file (`command.lock`, created atomically)
//! grants exactly one writer per IPC directory for the whole
//! request/response transaction. A crashed holder's lock is reclaimed only
//! after it goes stale, and only when no live client has touched it since.
//! Heartbeats are freshness-checked: a valid but old heartbeat never reads
//! as Ready.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use l4d2api_protocol as proto;
use thiserror::Error;

pub const COMMAND_FILE: &str = "command.txt";
pub const RESULT_FILE: &str = "result.txt";
pub const HEARTBEAT_FILE: &str = "heartbeat.txt";
pub const LOCK_FILE: &str = "command.lock";

/// Default: a heartbeat older than this never counts as live.
pub const DEFAULT_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);
/// Default: a lock untouched for this long may be reclaimed.
pub const DEFAULT_LOCK_STALE_AFTER: Duration = Duration::from_secs(30);
/// Total budget for retrying a rename blocked by a reader (Windows).
const RENAME_RETRY_BUDGET: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum Error {
    #[error("protocol error: {0}")]
    Protocol(#[from] proto::Error),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("another mutation is already outstanding (single-flight)")]
    Busy,
    #[error("another process holds the IPC write lock ({detail})")]
    Locked { detail: String },
    #[error("timed out waiting for the game reply after {0:?}")]
    Timeout(Duration),
    #[error("game generation changed (was {expected}, now {found}); handshake again")]
    StaleGeneration { expected: String, found: String },
    #[error("no heartbeat yet: game addon is missing or the game is closed")]
    NoHeartbeat,
    #[error("heartbeat is stale (last update {age:?}): game may be closed or frozen")]
    StaleHeartbeat { age: Duration },
    #[error("game is not ready: {reason}")]
    NotReady { reason: &'static str },
    #[error("action `{0}` is not advertised by the game")]
    UnsupportedCapability(String),
    #[error("game failed the request: {code} ({diag})")]
    GameRejected { code: String, diag: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    /// Addon present, host authority, gameplay ready.
    Ready,
    /// Addon present but not ready (menu, loading, not host, stale ...).
    NotReady { reason: &'static str },
    /// No valid heartbeat file.
    Absent,
    /// A heartbeat exists but is too old to trust.
    Stale { reason: &'static str },
    /// Fresh heartbeat from a compatible addon; the round is not active yet.
    Connected,
}

/// Session health model used by [`Client::status`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// No valid heartbeat file: addon missing or game closed.
    Disconnected,
    /// Fresh heartbeat, compatible version, round not active yet.
    Connected,
    /// Heartbeat too old (or tick stalled): do not treat the game as live.
    Stale { reason: &'static str },
    /// Fresh heartbeat but a hard gate fails (not host, version mismatch).
    NotReady { reason: &'static str },
    /// Host authority, active round, playable now.
    GameplayReady,
}

impl Health {
    pub fn is_ready(self) -> bool {
        matches!(self, Health::GameplayReady)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedResponse {
    pub status: proto::Status,
    pub code: String,
    pub requested: u32,
    pub actual: u32,
    pub diag: String,
    pub generation: String,
    pub request_id: String,
}

pub struct Client {
    dir: PathBuf,
    timeout: Duration,
    poll_interval: Duration,
    heartbeat_timeout: Duration,
    lock_stale_after: Duration,
    seq: u64,
    generation: Option<String>,
    capabilities: Vec<String>,
    addon_version: Option<String>,
    outstanding: bool,
    owner: String,
    last_tick: Option<(String, u64, Instant)>,
}

impl Client {
    pub fn new(dir: &Path, timeout: Duration, poll_interval: Duration) -> Client {
        Client {
            dir: dir.to_owned(),
            timeout,
            poll_interval,
            heartbeat_timeout: DEFAULT_HEARTBEAT_TIMEOUT,
            lock_stale_after: DEFAULT_LOCK_STALE_AFTER,
            seq: 0,
            generation: None,
            capabilities: Vec::new(),
            addon_version: None,
            outstanding: false,
            owner: format!(
                "pid{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ),
            last_tick: None,
        }
    }

    /// Override the heartbeat freshness window (default 5 s).
    pub fn set_heartbeat_timeout(&mut self, timeout: Duration) {
        self.heartbeat_timeout = timeout;
    }

    /// Override the lock-staleness window (default 30 s).
    pub fn set_lock_stale_after(&mut self, after: Duration) {
        self.lock_stale_after = after;
    }

    /// Configured IPC directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Pinned session generation, if [`Client::handshake`] ran.
    pub fn generation(&self) -> Option<&str> {
        self.generation.as_deref()
    }

    /// Capabilities advertised by the last handshake heartbeat.
    pub fn advertised_capabilities(&self) -> &[String] {
        &self.capabilities
    }

    /// A unique request id for this client instance. Callers must never
    /// reuse an id: stale results from an earlier request with the same id
    /// could otherwise satisfy a new wait.
    pub fn next_id(&mut self) -> String {
        self.seq += 1;
        format!(
            "r-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            self.seq
        )
    }

    fn read_record(&self, file: &str) -> Result<Option<String>, Error> {
        let path = self.dir.join(file);
        match fs::read_to_string(&path) {
            Ok(mut text) => {
                if (file == HEARTBEAT_FILE || file == RESULT_FILE) && text.ends_with('\0') {
                    text.pop();
                }
                Ok(Some(text))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::Io(error)),
        }
    }

    fn record_age(&self, file: &str) -> Option<Duration> {
        let meta = fs::metadata(self.dir.join(file)).ok()?;
        let modified = meta.modified().ok()?;
        SystemTime::now().duration_since(modified).ok()
    }

    /// Read and validate the current heartbeat, if any.
    pub fn heartbeat(&self) -> Result<Option<proto::Heartbeat>, Error> {
        match self.read_record(HEARTBEAT_FILE)? {
            None => Ok(None),
            Some(text) => match proto::decode_heartbeat(&text) {
                Ok(heartbeat) => Ok(Some(heartbeat)),
                Err(_) => Ok(None),
            },
        }
    }

    /// Read the heartbeat plus its file age. `None` when no valid heartbeat
    /// exists; the age tells freshness callers how old the snapshot is.
    pub fn heartbeat_fresh(&self) -> Result<Option<(proto::Heartbeat, Duration)>, Error> {
        let heartbeat = match self.heartbeat()? {
            None => return Ok(None),
            Some(heartbeat) => heartbeat,
        };
        let age = self.record_age(HEARTBEAT_FILE).unwrap_or(Duration::MAX);
        Ok(Some((heartbeat, age)))
    }

    /// Fresh handshake: requires a heartbeat and pins its generation.
    /// Clears any outstanding state from a previous game session.
    /// `connect` is the same handshake under the SDK-facing name.
    pub fn handshake(&mut self) -> Result<proto::Heartbeat, Error> {
        let heartbeat = self.heartbeat()?.ok_or(Error::NoHeartbeat)?;
        self.generation = Some(heartbeat.generation.clone());
        self.capabilities = heartbeat.capabilities.clone();
        self.addon_version = Some(heartbeat.addon_version.clone());
        self.outstanding = false;
        self.last_tick = Some((heartbeat.generation.clone(), heartbeat.tick, Instant::now()));
        Ok(heartbeat)
    }

    pub fn connect(&mut self) -> Result<proto::Heartbeat, Error> {
        self.handshake()
    }

    /// Capabilities from the most recent valid heartbeat read. Reads the
    /// file each call so long-lived clients see addon upgrades.
    pub fn capabilities(&self) -> Result<Vec<String>, Error> {
        Ok(self
            .heartbeat()?
            .map(|h| h.capabilities)
            .unwrap_or_default())
    }

    /// Current session health. Fresh heartbeats only: a valid but old
    /// heartbeat reports [`Health::Stale`], never ready. Observes `tick`
    /// progression across calls so a frozen game reads as stale even when
    /// its file timestamp was touched by something else.
    pub fn status(&mut self) -> Result<Health, Error> {
        let (heartbeat, age) = match self.heartbeat_fresh()? {
            None => return Ok(Health::Disconnected),
            Some(pair) => pair,
        };
        if age > self.heartbeat_timeout {
            return Ok(Health::Stale {
                reason: "heartbeat_old",
            });
        }
        let now = Instant::now();
        let stalled = matches!(
            &self.last_tick,
            Some((gen, tick, _)) if *gen == heartbeat.generation && *tick == heartbeat.tick
        );
        self.last_tick = Some((heartbeat.generation.clone(), heartbeat.tick, now));
        if stalled && age > self.poll_interval * 3 {
            return Ok(Health::Stale {
                reason: "tick_stalled",
            });
        }
        if heartbeat.addon_version.is_empty() {
            return Ok(Health::NotReady {
                reason: "protocol_mismatch",
            });
        }
        if !heartbeat.host_authority {
            return Ok(Health::NotReady { reason: "not_host" });
        }
        if !heartbeat.gameplay_ready || !heartbeat.round_active {
            return Ok(Health::Connected);
        }
        Ok(Health::GameplayReady)
    }

    /// Legacy connection view over [`Client::status`].
    pub fn connection(&self) -> Result<Connection, Error> {
        let (heartbeat, age) = match self.heartbeat_fresh()? {
            None => return Ok(Connection::Absent),
            Some(pair) => pair,
        };
        if age > self.heartbeat_timeout {
            return Ok(Connection::Stale {
                reason: "heartbeat_old",
            });
        }
        if !heartbeat.host_authority {
            return Ok(Connection::NotReady { reason: "not_host" });
        }
        if !heartbeat.gameplay_ready || !heartbeat.round_active {
            return Ok(Connection::NotReady {
                reason: "not_in_round",
            });
        }
        Ok(Connection::Ready)
    }

    /// Fail fast when the pinned session cannot run `action_name`: no
    /// handshake, stale heartbeat, generation drift, missing capability.
    pub fn ensure_ready_for(&mut self, action_name: &str) -> Result<(), Error> {
        let generation = self.generation.clone().ok_or(Error::NoHeartbeat)?;
        let (heartbeat, age) = self.heartbeat_fresh()?.ok_or(Error::NoHeartbeat)?;
        if age > self.heartbeat_timeout {
            return Err(Error::StaleHeartbeat { age });
        }
        if heartbeat.generation != generation {
            return Err(Error::StaleGeneration {
                expected: generation,
                found: heartbeat.generation,
            });
        }
        if !heartbeat.host_authority {
            return Err(Error::NotReady { reason: "not_host" });
        }
        if action_name != "test_connection"
            && !heartbeat.capabilities.iter().any(|c| c == action_name)
        {
            return Err(Error::UnsupportedCapability(action_name.to_owned()));
        }
        Ok(())
    }

    // --- cross-process single-writer lock -------------------------------

    fn lock_path(&self) -> PathBuf {
        self.dir.join(LOCK_FILE)
    }

    /// Try to become the single writer. Non-blocking: a live foreign lock
    /// is an error, never a wait. A stale lock is reclaimed only when its
    /// token is unchanged across two reads (no live client touched it).
    fn acquire_lock(&self) -> Result<(), Error> {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.lock_path())
        {
            Ok(mut file) => {
                use std::io::Write as _;
                let _ = writeln!(file, "{}", self.owner);
                Ok(())
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => self.reclaim_or_refuse(),
            Err(error) => Err(Error::Io(error)),
        }
    }

    fn reclaim_or_refuse(&self) -> Result<(), Error> {
        let first = fs::read_to_string(self.lock_path()).unwrap_or_default();
        let age = self.record_age(LOCK_FILE).unwrap_or(Duration::from_secs(0));
        if age <= self.lock_stale_after {
            return Err(Error::Locked {
                detail: format!("held for {age:?}"),
            });
        }
        // Stale candidate: re-read; a live client heartbeats its lock by
        // rewriting it, so a changed token means "hands off".
        std::thread::sleep(Duration::from_millis(25));
        let second = fs::read_to_string(self.lock_path()).unwrap_or_default();
        if second != first {
            return Err(Error::Locked {
                detail: "lock changed during reclaim; another client is live".into(),
            });
        }
        // Our token or nobody's: remove, then atomically recreate. A lost
        // race surfaces as AlreadyExists on create -> Locked, never as two
        // concurrent writers.
        let _ = fs::remove_file(self.lock_path());
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.lock_path())
        {
            Ok(mut file) => {
                use std::io::Write as _;
                let _ = writeln!(file, "{}", self.owner);
                Ok(())
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(Error::Locked {
                detail: "lost the reclaim race; another client is live".into(),
            }),
            Err(error) => Err(Error::Io(error)),
        }
    }

    /// Release the lock, but only when it still carries our token: never
    /// remove a lock a peer reclaimed after our own timeout.
    fn release_lock(&self) {
        if let Ok(token) = fs::read_to_string(self.lock_path()) {
            if token.trim_end() == self.owner {
                let _ = fs::remove_file(self.lock_path());
            }
        }
    }

    /// Atomically replace `command.txt`: temp file in the same directory,
    /// then rename over the destination. Retries cover only the
    /// filesystem replacement itself (a locked destination on Windows), a
    /// bounded number of times -- gameplay is never re-executed. Temp
    /// files are removed on every failure path.
    fn write_command_atomic(&self, text: &str, id: &str) -> Result<(), Error> {
        let target = self.dir.join(COMMAND_FILE);
        let tmp = self.dir.join(format!(
            "command.{}.{}.tmp",
            std::process::id(),
            sanitize_id(id)
        ));
        let write_result = fs::write(&tmp, text.as_bytes());
        if let Err(error) = write_result {
            let _ = fs::remove_file(&tmp);
            return Err(Error::Io(error));
        }
        // Windows fails the rename with access-denied while the game holds
        // command.txt open. Retry transient lock errors within a time budget.
        let deadline = std::time::Instant::now() + RENAME_RETRY_BUDGET;
        loop {
            match fs::rename(&tmp, &target) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    let retryable = matches!(
                        error.kind(),
                        io::ErrorKind::PermissionDenied
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    );
                    if !retryable || std::time::Instant::now() >= deadline {
                        let _ = fs::remove_file(&tmp);
                        return Err(Error::Io(error));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    /// Send one action and wait for its acknowledgement. Single-flight per
    /// instance plus a cross-process write lock held for the whole
    /// transaction. The `id` must be unique per request (see
    /// [`Client::next_id`]); a stale result never satisfies a new wait
    /// because replies must match id, sequence and generation together.
    pub fn send(&mut self, id: &str, action: proto::Action) -> Result<TypedResponse, Error> {
        if self.outstanding {
            return Err(Error::Busy);
        }
        let Some(generation) = self.generation.clone() else {
            return Err(Error::NoHeartbeat);
        };
        self.acquire_lock()?;
        self.outstanding = true;
        let result = self.send_locked(id, action, &generation);
        self.outstanding = false;
        self.release_lock();
        result
    }

    /// Convenience: generate a unique id, then [`Client::send`].
    pub fn send_auto(&mut self, action: proto::Action) -> Result<TypedResponse, Error> {
        let id = self.next_id();
        self.send(&id, action)
    }

    /// Ergonomic action helpers: each mints a unique request id, checks the
    /// advertised capability (mutations only) and waits for the game's
    /// typed reply. Rejections arrive as [`TypedResponse`] with a non-applied
    /// status -- never as an automatic retry.
    pub fn test_connection(&mut self) -> Result<TypedResponse, Error> {
        self.send_auto(proto::Action::TestConnection)
    }

    pub fn spawn_common(&mut self, count: u32) -> Result<TypedResponse, Error> {
        self.send_auto(proto::Action::SpawnCommon { count })
    }

    pub fn spawn_infected(&mut self, class: &str, count: u32) -> Result<TypedResponse, Error> {
        self.send_auto(proto::Action::SpawnInfected {
            class: class.to_owned(),
            count,
        })
    }

    pub fn spawn_item(&mut self, item: &str, count: u32) -> Result<TypedResponse, Error> {
        self.send_auto(proto::Action::SpawnItem {
            item: item.to_owned(),
            count,
        })
    }

    pub fn spawn_weapon(&mut self, weapon: &str, count: u32) -> Result<TypedResponse, Error> {
        self.send_auto(proto::Action::SpawnWeapon {
            weapon: weapon.to_owned(),
            count,
        })
    }

    fn send_locked(
        &mut self,
        id: &str,
        action: proto::Action,
        generation: &str,
    ) -> Result<TypedResponse, Error> {
        // Fail fast on capability/version drift before touching command.txt.
        let action_name = action.id().to_owned();
        self.ensure_ready_for(&action_name)?;
        // Recover the last sequence under the cross-process lock, including
        // commands whose reply timed out and whose outcome remains unknown.
        if let Some(text) = self.read_record(COMMAND_FILE)? {
            if let Ok(previous) = proto::decode_command(&text) {
                if previous.generation == generation {
                    self.seq = self.seq.max(previous.seq);
                }
            }
        }
        if let Some(text) = self.read_record(RESULT_FILE)? {
            if let Ok(previous) = proto::decode_result(&text) {
                if previous.generation == generation {
                    self.seq = self.seq.max(previous.seq);
                }
            }
        }
        self.seq += 1;
        let command = proto::Command {
            seq: self.seq,
            id: id.to_owned(),
            generation: generation.to_owned(),
            action,
        };
        let text = proto::encode_command(&command)?;
        self.write_command_atomic(&text, id)?;
        self.wait_for_reply(id, command.seq, generation)
    }

    fn wait_for_reply(&self, id: &str, seq: u64, generation: &str) -> Result<TypedResponse, Error> {
        let deadline = Instant::now() + self.timeout;
        loop {
            if let Some(text) = self.read_record(RESULT_FILE)? {
                if let Ok(record) = proto::decode_result(&text) {
                    // All three must match: a stale reply from an earlier
                    // request with a recycled id can never resolve this wait.
                    if record.id == id && record.seq == seq {
                        if record.generation != generation {
                            return Err(Error::StaleGeneration {
                                expected: generation.to_owned(),
                                found: record.generation,
                            });
                        }
                        return Ok(TypedResponse {
                            status: record.status,
                            code: record.code,
                            requested: record.requested,
                            actual: record.actual,
                            diag: record.diag,
                            generation: record.generation,
                            request_id: record.id,
                        });
                    }
                    // A reply for another id: keep waiting (no auto re-execute).
                }
            }
            // A generation change under our feet invalidates the wait.
            if let Ok(Some(heartbeat)) = self.heartbeat() {
                if heartbeat.generation != generation {
                    return Err(Error::StaleGeneration {
                        expected: generation.to_owned(),
                        found: heartbeat.generation,
                    });
                }
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout(self.timeout));
            }
            std::thread::sleep(self.poll_interval);
        }
    }
}

fn sanitize_id(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for byte in id.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' {
            out.push(byte as char);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("id");
    }
    if out.len() > 48 {
        out.truncate(48);
    }
    out
}

#[cfg(test)]
mod peer {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn test_dir(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pid = std::process::id();
        dir.push(format!("l4d2api-client-{name}-{pid}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn heartbeat_text(generation: &str, tick: u64) -> String {
        proto::encode_heartbeat(&proto::Heartbeat {
            generation: generation.to_owned(),
            addon_version: "0.1.0".into(),
            map: "c1m1_hotel".into(),
            round_active: true,
            host_authority: true,
            gameplay_ready: true,
            capabilities: vec![
                "test_connection".into(),
                "spawn_common".into(),
                "spawn_item".into(),
                "spawn_weapon".into(),
            ],
            tick,
        })
        .unwrap()
    }

    fn spawn_peer(dir: PathBuf) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            for _ in 0..2000 {
                if let Ok(text) = fs::read_to_string(dir.join(COMMAND_FILE)) {
                    if let Ok(command) = proto::decode_command(&text) {
                        let reply = proto::encode_result(&proto::ResultRecord {
                            seq: command.seq,
                            id: command.id.clone(),
                            generation: command.generation.clone(),
                            status: proto::Status::Applied,
                            code: "OK".into(),
                            requested: command.action.requested_count(),
                            actual: command.action.requested_count(),
                            diag: String::new(),
                        })
                        .unwrap();
                        fs::write(dir.join(RESULT_FILE), reply).unwrap();
                        return;
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("peer saw no command");
        })
    }

    #[test]
    fn handshake_and_request_ack() {
        let dir = test_dir("ack");
        fs::write(dir.join(HEARTBEAT_FILE), heartbeat_text("g-1", 1)).unwrap();
        let mut client = Client::new(&dir, Duration::from_secs(5), Duration::from_millis(5));

        let peer = spawn_peer(dir.clone());

        let heartbeat = client.handshake().unwrap();
        assert_eq!(heartbeat.generation, "g-1");
        let response = client
            .send("r_1", proto::Action::SpawnCommon { count: 3 })
            .unwrap();
        assert_eq!(response.status, proto::Status::Applied);
        assert_eq!(response.actual, 3);
        assert_eq!(response.request_id, "r_1");
        peer.join().unwrap();
    }

    #[test]
    fn timeout_is_not_success() {
        let dir = test_dir("timeout");
        fs::write(dir.join(HEARTBEAT_FILE), heartbeat_text("g-1", 1)).unwrap();
        let mut client = Client::new(&dir, Duration::from_millis(80), Duration::from_millis(10));
        client.handshake().unwrap();
        let error = client
            .send("r_9", proto::Action::TestConnection)
            .unwrap_err();
        assert!(matches!(error, Error::Timeout(_)));
    }

    #[test]
    fn generation_change_rejects_stale_wait() {
        let dir = test_dir("stale");
        fs::write(dir.join(HEARTBEAT_FILE), heartbeat_text("g-1", 1)).unwrap();
        let mut client = Client::new(&dir, Duration::from_secs(5), Duration::from_millis(5));
        client.handshake().unwrap();
        // Game restarts under our feet: new generation, no reply coming.
        fs::write(dir.join(HEARTBEAT_FILE), heartbeat_text("g-2", 2)).unwrap();
        let error = client
            .send("r_2", proto::Action::TestConnection)
            .unwrap_err();
        assert!(matches!(error, Error::StaleGeneration { .. }));
    }

    #[test]
    fn stale_heartbeat_never_reads_ready() {
        let dir = test_dir("stalehb");
        fs::write(dir.join(HEARTBEAT_FILE), heartbeat_text("g-1", 1)).unwrap();
        let mut client = Client::new(&dir, Duration::from_secs(5), Duration::from_millis(5));
        client.set_heartbeat_timeout(Duration::from_millis(40));
        client.handshake().unwrap();
        std::thread::sleep(Duration::from_millis(80));
        assert!(matches!(client.status().unwrap(), Health::Stale { .. }));
        assert!(matches!(
            client.connection().unwrap(),
            Connection::Stale { .. }
        ));
        let error = client
            .send("r_s1", proto::Action::TestConnection)
            .unwrap_err();
        assert!(matches!(error, Error::StaleHeartbeat { .. }));
    }

    #[test]
    fn duplicate_and_partial_replies_are_ignored() {
        let dir = test_dir("dup");
        fs::write(dir.join(HEARTBEAT_FILE), heartbeat_text("g-1", 1)).unwrap();
        let mut client = Client::new(&dir, Duration::from_secs(5), Duration::from_millis(5));
        client.handshake().unwrap();
        // A stale reply with the same id but a different sequence must not resolve the request.
        fs::write(
            dir.join(RESULT_FILE),
            proto::encode_result(&proto::ResultRecord {
                seq: 2,
                id: "r_new".into(),
                generation: "g-1".into(),
                status: proto::Status::Applied,
                code: "OK".into(),
                requested: 0,
                actual: 0,
                diag: String::new(),
            })
            .unwrap(),
        )
        .unwrap();
        let peer_dir = dir.clone();
        let peer = std::thread::spawn(move || {
            // Like the game: answer only the command that was actually written,
            // using its own sequence number. A reply to a command that was never
            // sent would race the client's sequence recovery in send_locked.
            let deadline = Instant::now() + Duration::from_secs(5);
            let command = loop {
                if let Ok(text) = fs::read_to_string(peer_dir.join(COMMAND_FILE)) {
                    if let Ok(command) = proto::decode_command(&text) {
                        if command.id == "r_new" {
                            break command;
                        }
                    }
                }
                assert!(Instant::now() < deadline, "peer saw no command");
                std::thread::sleep(Duration::from_millis(5));
            };
            std::thread::sleep(Duration::from_millis(50));
            let reply = proto::encode_result(&proto::ResultRecord {
                seq: command.seq,
                id: "r_new".into(),
                generation: "g-1".into(),
                status: proto::Status::Rejected,
                code: "NOT_IN_ROUND".into(),
                requested: 3,
                actual: 0,
                diag: "round not active".into(),
            })
            .unwrap();
            fs::write(peer_dir.join(RESULT_FILE), reply).unwrap();
        });
        let response = client
            .send("r_new", proto::Action::SpawnCommon { count: 3 })
            .unwrap();
        assert_eq!(response.status, proto::Status::Rejected);
        assert_eq!(response.code, "NOT_IN_ROUND");
        peer.join().unwrap();
    }

    #[test]
    fn second_writer_gets_locked_not_interleaved() {
        let dir = test_dir("lock");
        fs::write(dir.join(HEARTBEAT_FILE), heartbeat_text("g-1", 1)).unwrap();
        let mut first = Client::new(&dir, Duration::from_secs(5), Duration::from_millis(5));
        let mut second = Client::new(&dir, Duration::from_secs(5), Duration::from_millis(5));
        first.handshake().unwrap();
        second.handshake().unwrap();
        // Hold the lock manually: the second client must refuse, not write.
        first.acquire_lock().unwrap();
        let error = second
            .send("r_lock_1", proto::Action::TestConnection)
            .unwrap_err();
        assert!(matches!(error, Error::Locked { .. }));
        // No command file was written by the refused client.
        assert!(!dir.join(COMMAND_FILE).exists());
        first.release_lock();
        // After release the second client proceeds normally.
        let peer = spawn_peer(dir.clone());
        let response = second
            .send("r_lock_2", proto::Action::TestConnection)
            .unwrap();
        assert_eq!(response.status, proto::Status::Applied);
        peer.join().unwrap();
    }

    #[test]
    fn stale_lock_is_reclaimed_live_lock_is_not() {
        let dir = test_dir("reclaim");
        fs::create_dir_all(&dir).unwrap();
        let mut client = Client::new(&dir, Duration::from_secs(5), Duration::from_millis(5));
        client.set_lock_stale_after(Duration::from_millis(30));
        // Untouched foreign lock older than the window: reclaimable.
        fs::write(dir.join(LOCK_FILE), "ghost-1\n").unwrap();
        std::thread::sleep(Duration::from_millis(80));
        client.acquire_lock().unwrap();
        client.release_lock();
        assert!(!dir.join(LOCK_FILE).exists());
    }

    #[test]
    fn replacing_existing_command_file_succeeds() {
        let dir = test_dir("replace");
        fs::write(dir.join(HEARTBEAT_FILE), heartbeat_text("g-1", 1)).unwrap();
        // A previous command is still on disk: replacement must succeed.
        fs::write(dir.join(COMMAND_FILE), "stale-bytes").unwrap();
        let mut client = Client::new(&dir, Duration::from_secs(5), Duration::from_millis(5));
        client.handshake().unwrap();
        let peer = spawn_peer(dir.clone());
        let response = client
            .send("r_rep_1", proto::Action::TestConnection)
            .unwrap();
        assert_eq!(response.status, proto::Status::Applied);
        peer.join().unwrap();
        // No temp files leak after success.
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn unsupported_capability_fails_before_write() {
        let dir = test_dir("cap");
        fs::write(
            dir.join(HEARTBEAT_FILE),
            proto::encode_heartbeat(&proto::Heartbeat {
                generation: "g-1".into(),
                addon_version: "0.1.0".into(),
                map: "c1m1_hotel".into(),
                round_active: true,
                host_authority: true,
                gameplay_ready: true,
                capabilities: vec!["test_connection".into()],
                tick: 1,
            })
            .unwrap(),
        )
        .unwrap();
        let mut client = Client::new(&dir, Duration::from_secs(2), Duration::from_millis(5));
        client.handshake().unwrap();
        let error = client
            .send(
                "r_cap_1",
                proto::Action::SpawnItem {
                    item: "first_aid_kit".into(),
                    count: 1,
                },
            )
            .unwrap_err();
        assert!(matches!(error, Error::UnsupportedCapability(_)));
        assert!(!dir.join(COMMAND_FILE).exists());
    }
}
