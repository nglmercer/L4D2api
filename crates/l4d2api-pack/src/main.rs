//! `l4d2api-pack` CLI: pack `addon/` into a versioned, validated `.vpk`.

use std::path::PathBuf;
use std::process::ExitCode;

use l4d2api_pack::write_release;

fn usage() -> String {
    "usage: l4d2api-pack --addon <dir> --out <file.vpk> --version <x.y.z>".to_owned()
}

fn main() -> ExitCode {
    let mut addon: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut version: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addon" => addon = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from),
            "--version" => version = args.next(),
            _ => {
                eprintln!("{usage}", usage = usage());
                return ExitCode::from(2);
            }
        }
    }
    let (Some(addon), Some(out), Some(version)) = (addon, out, version) else {
        eprintln!("{usage}", usage = usage());
        return ExitCode::from(2);
    };
    match write_release(&addon, &out, &version) {
        Ok(release) => {
            println!("wrote {} ({} bytes)", out.display(), release.bytes.len());
            println!("sha256 {}", release.sha256);
            for entry in &release.entries {
                println!("  {}", entry.path);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("pack failed: {error}");
            ExitCode::from(1)
        }
    }
}
