//! Golden-fixture tests: decode the committed records in
//! `tests/fixtures/` and pin their exact field values.

use l4d2api_protocol as proto;
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../tests/fixtures");
    path.push(name);
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing fixture {name}"))
}

#[test]
fn valid_command_fixture() {
    let command = proto::decode_command(&fixture("command_valid.txt")).unwrap();
    assert_eq!(
        command,
        proto::Command {
            seq: 7,
            id: "r_12_7".into(),
            generation: "g-9f2c".into(),
            action: proto::Action::SpawnCommon { count: 5 },
        }
    );
}

#[test]
fn valid_result_fixture() {
    let result = proto::decode_result(&fixture("result_applied.txt")).unwrap();
    assert_eq!(result.status, proto::Status::Applied);
    assert_eq!(result.code, "OK");
    assert_eq!((result.requested, result.actual), (5, 5));
}

#[test]
fn valid_heartbeat_fixture() {
    let heartbeat = proto::decode_heartbeat(&fixture("heartbeat_ready.txt")).unwrap();
    assert_eq!(heartbeat.map, "c1m1_hotel");
    assert!(heartbeat.host_authority && heartbeat.gameplay_ready);
    assert_eq!(heartbeat.capabilities.len(), 3);
}

#[test]
fn rejected_not_host_fixture() {
    let result = proto::decode_result(&fixture("result_rejected_not_host.txt")).unwrap();
    assert_eq!(result.status, proto::Status::Rejected);
    assert_eq!(result.code, "NOT_HOST");
}

#[test]
fn old_generation_decodes_but_must_not_execute() {
    // Framing is valid; generation freshness is enforced by both sides
    // (client drops cross-generation replies, game ignores stale commands).
    let command = proto::decode_command(&fixture("expired_old_generation.txt")).unwrap();
    assert_eq!(command.generation, "g-old");
    assert_eq!(command.action, proto::Action::TestConnection);
}

#[test]
fn malformed_fixtures_rejected() {
    assert!(proto::decode_command(&fixture("malformed_bad_crc.txt")).is_err());
    assert!(proto::decode_command(&fixture("truncated_no_end.txt")).is_err());
    assert!(proto::decode_command(&fixture("malformed_duplicate_key.txt")).is_err());
    assert!(proto::decode_command(&fixture("malformed_unknown_action.txt")).is_err());
}
