//! `l4d2api-client`: session client over the game's `ems/tiktools_l4d2/` files.
//!
//! One outstanding mutation at a time (single-flight). Writes are atomic
//! (temp file + rename); reads validate the full record and ignore partial
//! or malformed snapshots. Timeouts never trigger an automatic re-execute:
//! the game may already have acted.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use l4d2api_protocol as proto;
use thiserror::Error;

pub const COMMAND_FILE: &str = "command.txt";
pub const RESULT_FILE: &str = "result.txt";
pub const HEARTBEAT_FILE: &str = "heartbeat.txt";

#[derive(Debug, Error)]
pub enum Error {
    #[error("protocol error: {0}")]
    Protocol(#[from] proto::Error),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("another mutation is already outstanding (single-flight)")]
    Busy,
    #[error("timed out waiting for the game reply after {0:?}")]
    Timeout(Duration),
    #[error("game generation changed (was {expected}, now {found}); handshake again")]
    StaleGeneration { expected: String, found: String },
    #[error("no heartbeat yet: game addon is missing or the game is closed")]
    NoHeartbeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    /// Addon present, host authority, gameplay ready.
    Ready,
    /// Addon present but not ready (menu, loading, not host, stale ...).
    NotReady { reason: &'static str },
    /// No valid heartbeat file.
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedResponse {
    pub status: proto::Status,
    pub code: String,
    pub requested: u32,
    pub actual: u32,
    pub diag: String,
    pub generation: String,
}

pub struct Client {
    dir: PathBuf,
    timeout: Duration,
    poll_interval: Duration,
    seq: u64,
    generation: Option<String>,
    outstanding: bool,
}

impl Client {
    pub fn new(dir: &Path, timeout: Duration, poll_interval: Duration) -> Client {
        Client {
            dir: dir.to_owned(),
            timeout,
            poll_interval,
            seq: 0,
            generation: None,
            outstanding: false,
        }
    }

    fn read_record(&self, file: &str) -> Result<Option<String>, Error> {
        let path = self.dir.join(file);
        match fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::Io(error)),
        }
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

    /// Fresh handshake: requires a heartbeat and pins its generation.
    /// Clears any outstanding state from a previous game session.
    pub fn handshake(&mut self) -> Result<proto::Heartbeat, Error> {
        let heartbeat = self.heartbeat()?.ok_or(Error::NoHeartbeat)?;
        self.generation = Some(heartbeat.generation.clone());
        self.outstanding = false;
        Ok(heartbeat)
    }

    pub fn connection(&self) -> Result<Connection, Error> {
        let Some(heartbeat) = self.heartbeat()? else {
            return Ok(Connection::Absent);
        };
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

    /// Send one action and wait for its acknowledgement. Single-flight:
    /// returns [`Error::Busy`] while a previous call is still outstanding.
    pub fn send(&mut self, id: &str, action: proto::Action) -> Result<TypedResponse, Error> {
        if self.outstanding {
            return Err(Error::Busy);
        }
        let Some(generation) = self.generation.clone() else {
            return Err(Error::NoHeartbeat);
        };
        self.seq += 1;
        let command = proto::Command {
            seq: self.seq,
            id: id.to_owned(),
            generation: generation.clone(),
            action,
        };
        let text = proto::encode_command(&command)?;
        // Atomic host-side write: temp file + rename.
        let tmp = self.dir.join(format!("command.{id}.tmp"));
        fs::write(&tmp, text.as_bytes())?;
        fs::rename(&tmp, self.dir.join(COMMAND_FILE))?;
        self.outstanding = true;
        let result = self.wait_for_reply(id, command.seq, &generation);
        self.outstanding = false;
        result
    }

    fn wait_for_reply(&self, id: &str, seq: u64, generation: &str) -> Result<TypedResponse, Error> {
        let deadline = Instant::now() + self.timeout;
        loop {
            if let Some(text) = self.read_record(RESULT_FILE)? {
                if let Ok(record) = proto::decode_result(&text) {
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
        dir.push(format!("l4d2api-client-{name}-{nanos}"));
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
            capabilities: vec!["spawn_common".into()],
            tick,
        })
        .unwrap()
    }

    #[test]
    fn handshake_and_request_ack() {
        let dir = test_dir("ack");
        fs::write(dir.join(HEARTBEAT_FILE), heartbeat_text("g-1", 1)).unwrap();
        let mut client = Client::new(&dir, Duration::from_secs(5), Duration::from_millis(5));

        // Fake VScript peer: answer whatever valid command appears.
        let peer_dir = dir.clone();
        let peer = std::thread::spawn(move || {
            for _ in 0..1000 {
                if let Ok(text) = fs::read_to_string(peer_dir.join(COMMAND_FILE)) {
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
                        fs::write(peer_dir.join(RESULT_FILE), reply).unwrap();
                        return;
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("peer saw no command");
        });

        let heartbeat = client.handshake().unwrap();
        assert_eq!(heartbeat.generation, "g-1");
        let response = client
            .send("r_1", proto::Action::SpawnCommon { count: 3 })
            .unwrap();
        assert_eq!(response.status, proto::Status::Applied);
        assert_eq!(response.actual, 3);
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
            std::thread::sleep(Duration::from_millis(50));
            let reply = proto::encode_result(&proto::ResultRecord {
                seq: 1,
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
}
