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
    assert!(proto::decode_command(&fixture("truncated_partial.txt")).is_err());
    assert!(proto::decode_command(&fixture("malformed_duplicate_key.txt")).is_err());
    assert!(proto::decode_command(&fixture("malformed_unknown_action.txt")).is_err());
    assert!(proto::decode_command(&fixture("malformed_unknown_key.txt")).is_err());
    assert!(proto::decode_command(&fixture("malformed_invalid_integer.txt")).is_err());
    assert!(proto::decode_command(&fixture("malformed_invalid_params.txt")).is_err());
    assert!(proto::decode_command(&fixture("unsupported_version.txt")).is_err());
    assert!(proto::decode_command(&fixture("oversized_record.txt")).is_err());
}

#[test]
fn per_action_command_fixtures() {
    let test = proto::decode_command(&fixture("command_test_connection.txt")).unwrap();
    assert_eq!(test.action, proto::Action::TestConnection);
    let item = proto::decode_command(&fixture("command_spawn_item.txt")).unwrap();
    assert_eq!(
        item.action,
        proto::Action::SpawnItem {
            item: "first_aid_kit".into(),
            count: 2,
        }
    );
    let weapon = proto::decode_command(&fixture("command_spawn_weapon.txt")).unwrap();
    assert_eq!(
        weapon.action,
        proto::Action::SpawnWeapon {
            weapon: "weapon_rifle_ak47".into(),
            count: 1,
        }
    );
    let infected = proto::decode_command(&fixture("command_spawn_infected.txt")).unwrap();
    assert_eq!(
        infected.action,
        proto::Action::SpawnInfected {
            class: "boomer".into(),
            count: 3,
        }
    );
    // Round-trip: every valid command fixture re-encodes byte-identically.
    for name in [
        "command_valid.txt",
        "command_test_connection.txt",
        "command_spawn_item.txt",
        "command_spawn_weapon.txt",
        "command_spawn_infected.txt",
    ] {
        let raw = fixture(name);
        let command = proto::decode_command(&raw).unwrap();
        assert_eq!(proto::encode_command(&command).unwrap(), raw, "{name}");
    }
}

#[test]
fn partial_result_fixture_reports_verified_actual() {
    let result = proto::decode_result(&fixture("result_partial.txt")).unwrap();
    assert_eq!(result.status, proto::Status::Partial);
    assert_eq!(result.code, "PARTIAL");
    assert!((result.actual, result.requested) == (3, 5));
}

#[test]
fn session_level_fixtures_decode_but_must_not_execute() {
    // Framing is valid by design; freshness/ordering/duplication are
    // enforced by session logic (client generation checks, game replay
    // protection), not by the codec.
    for name in [
        "expired_old_generation.txt",
        "out_of_order_command.txt",
        "duplicate_request.txt",
    ] {
        proto::decode_command(&fixture(name))
            .unwrap_or_else(|_| panic!("{name} must stay framing-valid"));
    }
}
