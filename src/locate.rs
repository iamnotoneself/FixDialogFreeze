//! 目标 DLL 定位：拖入参数 -> Epic 游戏清单 -> 卸载注册表 -> 硬盘两层扫描

use std::fs;
use std::path::{Path, PathBuf};

use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};

/// 游戏内 DLL 的相对路径
pub const REL: &str = r"thedarksidedetective_Data\Managed\SpookyDoorway.DarksideShared.dll";

/// 把用户拖进来的路径解析成目标 DLL
pub fn resolve_candidate(target: Option<&str>) -> Option<PathBuf> {
    let t = target?.trim().trim_matches('"').trim();
    if t.is_empty() {
        return None;
    }
    let p = Path::new(t);
    if p.is_file() {
        return Some(p.to_path_buf());
    }
    if p.is_dir() {
        for rel in [REL, r"SpookyDoorway.DarksideShared.dll", r"Managed\SpookyDoorway.DarksideShared.dll"] {
            let candidate = p.join(rel);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// 自动查找：拖入参数 -> Epic 清单 -> 注册表 -> 硬盘扫描
pub fn find_game_dll(arg: Option<&str>) -> Option<PathBuf> {
    resolve_candidate(arg)
        .or_else(from_epic)
        .or_else(from_registry)
        .or_else(scan_drives)
}

/// 从 Epic 游戏清单里找安装位置
fn from_epic() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(std::env::var_os("ProgramData")?)
        .join(r"Epic\EpicGamesLauncher\Data\Manifests");
    for entry in fs::read_dir(manifest_dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("item") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else { continue };
        for line in text.lines() {
            if let Some(location) = line.trim().strip_prefix("InstallLocation=") {
                let candidate = Path::new(location.trim()).join(REL);
                if candidate.is_file() {
                    return Some(candidate);
                }
                break;
            }
        }
    }
    None
}

/// 从“已安装程序”注册表项里找（DisplayName 含 Darkside）
fn from_registry() -> Option<PathBuf> {
    const SUB: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    const SUB32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

    let attempts = [
        (HKEY_LOCAL_MACHINE, SUB, KEY_READ | KEY_WOW64_64KEY),
        (HKEY_LOCAL_MACHINE, SUB32, KEY_READ | KEY_WOW64_32KEY),
        (HKEY_CURRENT_USER, SUB, KEY_READ),
    ];

    for (hive, sub, flags) in attempts {
        let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(sub, flags) else { continue };
        for name in root.enum_keys().flatten() {
            let Ok(item) = root.open_subkey_with_flags(&name, flags) else { continue };
            let Ok(display) = item.get_value::<String, _>("DisplayName") else { continue };
            if !display.to_lowercase().contains("darkside") {
                continue;
            }
            let Ok(location) = item.get_value::<String, _>("InstallLocation") else { continue };
            if location.trim().is_empty() {
                continue;
            }
            let candidate = Path::new(location.trim()).join(REL);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// 硬盘两层扫描：找名字含 Darkside 的目录
fn scan_drives() -> Option<PathBuf> {
    const SKIP: [&str; 5] = ["Windows", "$Recycle.Bin", "System Volume Information", "Recovery", "Config.Msi"];

    for letter in b'C'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", letter as char));
        let Ok(entries) = fs::read_dir(&root) else { continue };

        let mut level1 = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains("darkside") {
                let candidate = path.join(REL);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            if !SKIP.iter().any(|s| name == s.to_lowercase()) {
                level1.push(path);
            }
        }

        for dir in level1 {
            let Ok(entries) = fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                if !entry.path().is_dir() {
                    continue;
                }
                if !entry.file_name().to_string_lossy().to_lowercase().contains("darkside") {
                    continue;
                }
                let candidate = entry.path().join(REL);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}
