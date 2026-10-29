//! `l4d2api-installer` CLI: game-dir checks and owned file operations.
//!
//! All paths are identical on Windows and Linux; only the default Steam
//! location differs (discovered automatically). `check`, `verify` and
//! `status` are read-only. `--json` prints one machine-readable object per
//! command; the default human output and all exit codes are unchanged.

use std::path::PathBuf;
use std::process::ExitCode;

use l4d2api_installer as installer;

fn usage() -> String {
    "usage:\n\
     \tl4d2api-installer [--json] check [--game-dir <dir>]\n\
     \tl4d2api-installer [--json] status [--game-dir <dir>]\n\
     \tl4d2api-installer [--json] verify --game-dir <dir>\n\
     \tl4d2api-installer install --game-dir <dir> --vpk <file.vpk> --version <x.y.z>\n\
     \tl4d2api-installer repair --game-dir <dir> --vpk <file.vpk> --version <x.y.z>\n\
     \tl4d2api-installer uninstall --game-dir <dir>\n\
     \tl4d2api-installer quarantine --game-dir <dir> <addon-rel>\n\
     \tl4d2api-installer restore --game-dir <dir>"
        .to_owned()
}

fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
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

fn cmd_check(game_dir: Option<PathBuf>, json: bool) -> Result<bool, String> {
    let game_dir = resolve_game_dir(game_dir)?;
    let conflicts = installer::scan_addon_conflicts(&game_dir).map_err(|e| e.to_string())?;
    if json {
        let mut entries = String::new();
        for conflict in &conflicts {
            if !entries.is_empty() {
                entries.push(',');
            }
            let kind = match conflict.kind {
                installer::ConflictKind::WeaponScript => "weapon_script",
                installer::ConflictKind::StockScript => "stock_script",
            };
            entries.push_str(&format!(
                "{{\"addon\":\"{}\",\"stock\":\"{}\",\"kind\":\"{kind}\"}}",
                json_escape(&conflict.addon_rel),
                json_escape(&conflict.stock_path)
            ));
        }
        println!("{{\"conflicts\":[{entries}]}}");
    } else if conflicts.is_empty() {
        println!("no script conflicts");
    } else {
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
    }
    Ok(!conflicts
        .iter()
        .any(|conflict| matches!(conflict.kind, installer::ConflictKind::WeaponScript)))
}

fn cmd_status(game_dir: Option<PathBuf>, json: bool) -> Result<(), String> {
    let game_dir = resolve_game_dir(game_dir)?;
    let status = installer::status(&game_dir).map_err(|e| e.to_string())?;
    if json {
        let installed = status.installed.as_deref().unwrap_or("");
        let manifest = status.manifest_path.as_deref().unwrap_or("");
        let bad: Vec<String> = status
            .bad_files
            .iter()
            .map(|f| format!("\"{}\"", json_escape(f)))
            .collect();
        println!(
            "{{\"installed\":\"{}\",\"manifest\":\"{}\",\"valid\":{},\"bad_files\":[{}],\"quarantined\":{},\"conflicts\":{}}}",
            json_escape(installed),
            json_escape(manifest),
            status.valid,
            bad.join(","),
            status.quarantined,
            status.conflicts,
        );
    } else {
        match &status.installed {
            Some(version) => println!("installed version {version}"),
            None => println!("not installed"),
        }
        println!("valid: {}", status.valid);
        for file in &status.bad_files {
            println!("bad file: {file}");
        }
        println!("quarantined: {}", status.quarantined);
        println!("conflicts: {}", status.conflicts);
    }
    Ok(())
}

fn cmd_verify(game_dir: Option<PathBuf>, json: bool) -> Result<bool, String> {
    let game_dir = resolve_game_dir(game_dir)?;
    let bad = installer::verify(&game_dir).map_err(|e| e.to_string())?;
    if json {
        let files: Vec<String> = bad
            .iter()
            .map(|f| format!("\"{}\"", json_escape(f)))
            .collect();
        println!(
            "{{\"valid\":{},\"bad_files\":[{}]}}",
            bad.is_empty(),
            files.join(",")
        );
    } else if bad.is_empty() {
        println!("installation valid");
    } else {
        for file in &bad {
            println!("bad file: {file}");
        }
    }
    Ok(bad.is_empty())
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

fn cmd_repair(game_dir: Option<PathBuf>, vpk: PathBuf, version: String) -> Result<(), String> {
    let game_dir = resolve_game_dir(game_dir)?;
    let bytes = std::fs::read(&vpk).map_err(|e| format!("{}: {e}", vpk.display()))?;
    let plan = installer::repair(&game_dir, &bytes, &version).map_err(|e| e.to_string())?;
    println!(
        "repaired {} version {}",
        plan.vpk_rel, plan.manifest.version
    );
    Ok(())
}

fn cmd_uninstall(game_dir: Option<PathBuf>) -> Result<(), String> {
    let game_dir = resolve_game_dir(game_dir)?;
    let removed = installer::uninstall(&game_dir).map_err(|e| e.to_string())?;
    if removed.is_empty() {
        println!("nothing to remove");
    } else {
        for rel in removed {
            println!("removed {rel}");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    // Accept `--json` anywhere (before or after the subcommand).
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let json = raw.iter().any(|arg| arg == "--json");
    let mut args = raw.into_iter().filter(|arg| arg != "--json").peekable();
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
            cmd_check(game_dir, json).map(|clean| {
                if !clean {
                    std::process::exit(1);
                }
            })
        }
        "status" if positional.is_empty() && vpk.is_none() && version.is_none() => {
            cmd_status(game_dir, json)
        }
        "verify" if positional.is_empty() && vpk.is_none() && version.is_none() => {
            cmd_verify(game_dir, json).map(|valid| {
                if !valid {
                    std::process::exit(1);
                }
            })
        }
        "quarantine" if positional.len() == 1 && vpk.is_none() && version.is_none() && !json => {
            cmd_quarantine(game_dir, &positional[0])
        }
        "restore" if positional.is_empty() && vpk.is_none() && version.is_none() && !json => {
            cmd_restore(game_dir)
        }
        "install" if positional.is_empty() && !json => match (vpk, version) {
            (Some(vpk), Some(version)) => cmd_install(game_dir, vpk, version),
            _ => Err("install needs --vpk <file.vpk> --version <x.y.z>".into()),
        },
        "repair" if positional.is_empty() && !json => match (vpk, version) {
            (Some(vpk), Some(version)) => cmd_repair(game_dir, vpk, version),
            _ => Err("repair needs --vpk <file.vpk> --version <x.y.z>".into()),
        },
        "uninstall" if positional.is_empty() && vpk.is_none() && version.is_none() && !json => {
            cmd_uninstall(game_dir)
        }
        _ => Err(usage()),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
