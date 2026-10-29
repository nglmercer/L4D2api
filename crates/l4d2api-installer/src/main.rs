//! `l4d2api-installer` CLI: game-dir checks and owned file operations.
//!
//! All paths are identical on Windows and Linux; only the default Steam
//! location differs (discovered automatically). `check` is read-only.

use std::path::PathBuf;
use std::process::ExitCode;

use l4d2api_installer as installer;

fn usage() -> String {
    "usage:\n\
     \tl4d2api-installer check [--game-dir <dir>]\n\
     \tl4d2api-installer quarantine --game-dir <dir> <addon-rel>\n\
     \tl4d2api-installer restore --game-dir <dir>\n\
     \tl4d2api-installer install --game-dir <dir> --vpk <file.vpk> --version <x.y.z>"
        .to_owned()
}

fn resolve_game_dir(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(dir) = explicit {
        installer::validate_game_dir(&dir)
            .map_err(|error| format!("{}: {error}", dir.display()))?;
        return Ok(dir);
    }
    let roots = installer::candidate_steam_roots(None);
    let mut installs = installer::find_l4d2_installs(&roots);
    installs.sort();
    installs
        .into_iter()
        .next()
        .ok_or_else(|| "no L4D2 install found; pass --game-dir <folder>".to_owned())
}

fn cmd_check(game_dir: Option<PathBuf>) -> Result<bool, String> {
    let game_dir = resolve_game_dir(game_dir)?;
    let conflicts = installer::scan_addon_conflicts(&game_dir).map_err(|e| e.to_string())?;
    if conflicts.is_empty() {
        println!("no script conflicts");
        return Ok(true);
    }
    for conflict in &conflicts {
        let level = match conflict.kind {
            installer::ConflictKind::WeaponScript => "KICK",
            installer::ConflictKind::StockScript => "SUSPICIOUS",
        };
        println!(
            "[{level}] {} overrides {}",
            conflict.addon_rel, conflict.stock_path
        );
    }
    Ok(!conflicts
        .iter()
        .any(|conflict| matches!(conflict.kind, installer::ConflictKind::WeaponScript)))
}

fn cmd_quarantine(game_dir: Option<PathBuf>, addon_rel: &str) -> Result<(), String> {
    let game_dir = resolve_game_dir(game_dir)?;
    let backup = installer::quarantine_addon(&game_dir, addon_rel).map_err(|e| e.to_string())?;
    println!("quarantined {addon_rel} (backup {backup})");
    Ok(())
}

fn cmd_restore(game_dir: Option<PathBuf>) -> Result<(), String> {
    let game_dir = resolve_game_dir(game_dir)?;
    let restored = installer::restore_quarantined(&game_dir).map_err(|e| e.to_string())?;
    if restored.is_empty() {
        println!("nothing quarantined");
    } else {
        for rel in restored {
            println!("restored {rel}");
        }
    }
    Ok(())
}

fn cmd_install(game_dir: Option<PathBuf>, vpk: PathBuf, version: String) -> Result<(), String> {
    let game_dir = resolve_game_dir(game_dir)?;
    let bytes = std::fs::read(&vpk).map_err(|e| format!("{}: {e}", vpk.display()))?;
    let plan = installer::install(&game_dir, &bytes, &version).map_err(|e| e.to_string())?;
    println!(
        "installed {} version {}",
        plan.vpk_rel, plan.manifest.version
    );
    Ok(())
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let mut game_dir: Option<PathBuf> = None;
    let mut vpk: Option<PathBuf> = None;
    let mut version: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game-dir" => game_dir = args.next().map(PathBuf::from),
            "--vpk" => vpk = args.next().map(PathBuf::from),
            "--version" => version = args.next(),
            other => positional.push(other.to_owned()),
        }
    }
    let result = match command.as_str() {
        "check" if positional.is_empty() && vpk.is_none() && version.is_none() => {
            cmd_check(game_dir).map(|clean| {
                if !clean {
                    std::process::exit(1);
                }
            })
        }
        "quarantine" if positional.len() == 1 && vpk.is_none() && version.is_none() => {
            cmd_quarantine(game_dir, &positional[0])
        }
        "restore" if positional.is_empty() && vpk.is_none() && version.is_none() => {
            cmd_restore(game_dir)
        }
        "install" if positional.is_empty() => match (vpk, version) {
            (Some(vpk), Some(version)) => cmd_install(game_dir, vpk, version),
            _ => Err("install needs --vpk <file.vpk> --version <x.y.z>".into()),
        },
        _ => Err(usage()),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
