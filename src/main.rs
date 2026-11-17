//! The Darkside Detective（黑暗侦探）对话框卡死 一键修复工具
//!
//! Bug 原因：
//!   游戏脚本 SpookyDoorway.DarksideShared.dll 中的协程 PunctuationStartRule
//!   格式化台词时执行 `lastString.Insert(startCharIdx - 1, "\n")`。
//!   当台词首字符就是断行标点（如引号）时 startCharIdx = 0，index = -1
//!   抛出 ArgumentOutOfRangeException，协程死亡，对话框停留在“异步格式化中”
//!   状态且永远无法退出 —— 于是按哪里都没反应。
//!
//! 修复方式：
//!   在 Insert 前加入边界保护（startCharIdx <= 0 时跳过插入），同时删除一段
//!   无用的死代码腾出空间。就地改写方法体，文件总长度不变，不影响 DLL 中
//!   其它任何代码。修复前自动备份为 原文件名.dll.bak。

mod locate;
mod patch;

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

fn main() {
    let code = run();
    pause();
    std::process::exit(code);
}

fn run() -> i32 {
    let arg = std::env::args().nth(1);

    let Some(dll) = find_target(arg.as_deref()) else {
        println!("[失败] 仍未找到 SpookyDoorway.DarksideShared.dll。");
        println!("请打开游戏安装目录，把该 DLL 或整个游戏文件夹拖到本程序上后重试。");
        return 1;
    };

    println!("目标文件: {}", dll.display());

    let bytes = match fs::read(&dll) {
        Ok(b) => b,
        Err(e) => {
            println!("[失败] 无法读取文件：{e}");
            return 1;
        }
    };

    if bytes.len() < patch::REGION_OFF + patch::REGION_LEN {
        println!("[失败] 文件长度异常，可能不是预期的游戏版本，未做任何修改。");
        return 2;
    }

    let hash = patch::region_hash(&bytes);
    let bak = PathBuf::from(format!("{}.bak", dll.display()));

    // —— 已修复过 ——
    if hash == patch::HASH_PATCHED {
        println!("[信息] 该文件已经打过补丁，无需重复修复。");
        if bak.is_file()
            && fs::read(&bak).ok().map(|b| b.len() >= patch::REGION_OFF + patch::REGION_LEN
                && patch::region_hash(&b) == patch::HASH_ORIGINAL)
                == Some(true)
            && ask("输入 R 还原为原版（直接回车退出）: ")
                .is_some_and(|s| s.eq_ignore_ascii_case("r"))
        {
            match fs::read(&bak).map_err(|e| e.to_string()).and_then(|b| fs::write(&dll, b).map_err(|e| e.to_string())) {
                Ok(()) => println!("[成功] 已还原为原版。"),
                Err(e) => println!("[失败] 还原出错：{e}"),
            }
        }
        return 0;
    }

    // —— 无法识别的版本：安全退出 ——
    if hash != patch::HASH_ORIGINAL {
        println!("[失败] 无法识别的文件版本（游戏可能已更新，或与本工具适配的版本不一致）。");
        println!("为安全起见未做任何修改。请到本工具的 GitHub 仓库提交 Issue，");
        println!("注明游戏平台（Epic / Steam 等）与游戏版本。");
        return 2;
    }

    // —— 备份（已有可用备份则保留） ——
    let bak_ok = bak.is_file()
        && fs::read(&bak).ok().map(|b| b.len() >= patch::REGION_OFF + patch::REGION_LEN
            && patch::region_hash(&b) == patch::HASH_ORIGINAL)
            == Some(true);
    if !bak_ok
        && let Err(e) = fs::copy(&dll, &bak)
    {
        println!("[失败] 备份失败：{e}");
        return 3;
    }

    // —— 打补丁 ——
    let region = patch::build_region(&bytes);
    let mut patched = bytes.clone();
    patched[patch::REGION_OFF..patch::REGION_OFF + patch::REGION_LEN].copy_from_slice(&region);
    if let Err(e) = fs::write(&dll, &patched) {
        if e.kind() == io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32) {
            println!("[失败] 无法写入文件，游戏可能正在运行，请先关闭游戏后重试。");
        } else {
            println!("[失败] {e}");
        }
        return 3;
    }

    // —— 写入后校验 ——
    match fs::read(&dll) {
        Ok(verify) if patch::region_hash(&verify) == patch::HASH_PATCHED => {}
        Ok(_) => {
            if fs::copy(&bak, &dll).is_ok() {
                println!("[失败] 写入后校验不通过，已自动还原备份，未做任何改动。");
            } else {
                println!("[失败] 写入后校验不通过！请手动用备份还原。");
            }
            return 5;
        }
        Err(e) => {
            println!("[失败] 校验出错：{e}");
            return 5;
        }
    }

    println!("[成功] 修复完成！");
    println!("  已修改: {}", dll.display());
    println!("  原文件备份: {}", bak.display());
    println!("  现在可以启动游戏验证，之前卡住的对话场景不会再出现。");
    println!("  提示：若 Epic 的“验证/修复”还原本文件，重新运行本工具即可。");
    0
}

/// 先尝试参数/自动查找，找不到再提示用户手动输入路径
fn find_target(arg: Option<&str>) -> Option<std::path::PathBuf> {
    let found = locate::find_game_dll(arg);
    if found.is_some() {
        return found;
    }

    println!("[提示] 自动查找未找到游戏文件。");
    let input = ask("请把 SpookyDoorway.DarksideShared.dll 或游戏文件夹拖到本窗口后回车: ")?;
    locate::resolve_candidate(Some(&input))
}

/// 输出提示并读取一行输入（EOF 时返回 None）
fn ask(prompt: &str) -> Option<String> {
    print!("{prompt}");
    io::stdout().flush().ok()?;
    let mut line = String::new();
    match io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}

/// 结束前停留，方便双击运行时看清结果（管道输入时会立即返回）
fn pause() {
    println!();
    print!("按回车键退出...");
    io::stdout().flush().ok();
    let mut _line = String::new();
    let _ = io::stdin().read_line(&mut _line);
}
