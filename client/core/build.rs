use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let out_dir = env::var("OUT_DIR").unwrap_or_default();

    if target != "x86_64-pc-windows-gnu" {
        return;
    }

    let out_path = PathBuf::from(out_dir);
    let target_dir = match locate_target_dir(&out_path) {
        Some(path) => path,
        None => return,
    };

    let dll_name = "client_core.dll";
    let dll_path = target_dir.join("x86_64-pc-windows-gnu").join(&profile).join(dll_name);
    if !dll_path.exists() {
        return;
    }

    let repo_root = target_dir.parent().map(Path::to_path_buf).unwrap_or(target_dir);
    let dest_dir = repo_root.join("client").join("godot").join("bin").join("windows");
    if let Err(err) = fs::create_dir_all(&dest_dir) {
        println!("cargo:warning=failed to create output dir: {}", err);
        return;
    }
    let dest_path = dest_dir.join(dll_name);
    if let Err(err) = fs::copy(&dll_path, &dest_path) {
        println!("cargo:warning=failed to copy dll: {}", err);
        return;
    }
}

fn locate_target_dir(out_dir: &Path) -> Option<PathBuf> {
    // OUT_DIR: <target>/<triple>/<profile>/build/<crate>/out
    let mut cur = out_dir;
    for _ in 0..6 {
        if cur.ends_with("target") {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
    None
}
