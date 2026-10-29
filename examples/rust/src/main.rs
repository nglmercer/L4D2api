//! Basic L4D2api session over file IPC: connect, check capabilities,
//! ping, and spawn items behind a capability gate.
//!
//! Run from the repo root:
//!
//! ```sh
//! cargo run -p l4d2api-example
//! ```
//!
//! Against a real game this uses the default Steam install; override with
//! `--game-dir <path>` (or set `L4D2_GAME_DIR`).

use std::path::PathBuf;
use std::time::Duration;

use l4d2api_client::{Client, Health};

fn default_game_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("L4D2_GAME_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(target_os = "windows")]
    {
        let program_files =
            std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into());
        PathBuf::from(program_files)
            .join("Steam")
            .join("steamapps")
            .join("common")
            .join("Left 4 Dead 2")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/tmp/l4d2-mock-game")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut game_dir = default_game_dir();
    while let Some(arg) = args.next() {
        if arg == "--game-dir" {
            if let Some(dir) = args.next() {
                game_dir = PathBuf::from(dir);
            }
        }
    }

    let ipc_dir = game_dir.join("left4dead2/ems/tiktools_l4d2");

    let mut client = Client::new(&ipc_dir, Duration::from_secs(5), Duration::from_millis(250));

    let heartbeat = client.connect()?;
    println!("Map: {}", heartbeat.map);
    println!("Capabilities: {:?}", heartbeat.capabilities);

    match client.status()? {
        Health::GameplayReady => println!("Session: gameplay ready"),
        other => println!("Session: {other:?}"),
    }

    let ping = client.test_connection()?;
    println!("Connection: {:?} {}", ping.status, ping.code);

    if heartbeat.capabilities.iter().any(|cap| cap == "spawn_item") {
        let result = client.spawn_item("first_aid_kit", 2)?;
        println!("Spawned items: {} ({:?})", result.actual, result.status);
    } else {
        println!("spawn_item not advertised; skipping (no blind execution)");
    }

    Ok(())
}
