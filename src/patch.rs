//! 补丁核心：只对 DLL 中已验证的那一段 IL 方法体做就地改写
//!
//! 原版 PowerShell 实现用固定偏移 + 区域哈希来定位，这里保持一致，
//! 这样无论游戏装在哪里、文件路径是什么，只要版本匹配就能安全修复。

use sha2::{Digest, Sha256};

/// 已验证的方法体区域（文件偏移 0x136A4，长 334 字节）
pub const REGION_OFF: usize = 0x1_36A4;
pub const REGION_LEN: usize = 334;

/// 原始（有 bug）区域哈希
pub const HASH_ORIGINAL: &str = "e4c78062dd4af2eb85db85943ebaa22c324bde12de9d1d3e08ee4fedeaee2720";
/// 打完补丁后的区域哈希
pub const HASH_PATCHED: &str = "cebe52902487fb8e4b5e8b5a51c686e0728604ac1249cfbe20c49f82a5861914";

/// 计算文件该区域的 SHA-256，返回小写十六进制字符串
pub fn region_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(&bytes[REGION_OFF..REGION_OFF + REGION_LEN]);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 生成长度为 334 的补丁区域
///
/// 步骤与原实现一致：
/// 1. 删除偏移 [0x8C, 0x92) 的死代码；
/// 2. 在 0xAC 处插入守卫 `ldloc.s 6; ldc.i4.0; ble.s skip`；
/// 3. 修正因位移而变化的分支偏移；
/// 4. 把 fat header 里的 code_size 从 322 改成 321，并在末尾补一个填充字节。
pub fn build_region(file: &[u8]) -> Vec<u8> {
    let region = &file[REGION_OFF..REGION_OFF + REGION_LEN];
    let mut hdr = region[..12].to_vec();
    let code = &region[12..]; // 322 字节

    let guard: [u8; 5] = [0x11, 0x06, 0x16, 0x31, 0x1A];

    let mut new_code = Vec::with_capacity(321);
    new_code.extend_from_slice(&code[0..=0x8B]);
    new_code.extend_from_slice(&code[0x92..=0xB1]);
    new_code.extend_from_slice(&guard);
    new_code.extend_from_slice(&code[0xB2..=0x141]);
    debug_assert_eq!(new_code.len(), 321);

    // —— 修正因位移而变化的分支偏移 ——
    new_code[0x14] = 0x20;
    new_code[0x15] = 0x01;
    new_code[0x16] = 0x00;
    new_code[0x17] = 0x00; // beq    +0x120
    new_code[0x3A] = 0xA1;
    new_code[0x3B] = 0x00;
    new_code[0x3C] = 0x00;
    new_code[0x3D] = 0x00; // br     +0xA1
    new_code[0x97] = 0x39; // br.s
    new_code[0xAB] = 0x1F; // bne.un.s
    new_code[0xB0] = 0x1A; // ble.s（守卫）
    new_code[0xD8] = 0xBF; // blt.s
    new_code[0xE5] = 0x55;
    new_code[0xE6] = 0xFF;
    new_code[0xE7] = 0xFF;
    new_code[0xE8] = 0xFF; // blt

    // —— fat header 中的 code_size：322 -> 321 ——
    hdr[4] = 0x41;
    hdr[5] = 0x01;
    hdr[6] = 0x00;
    hdr[7] = 0x00;

    let mut out = Vec::with_capacity(REGION_LEN);
    out.extend_from_slice(&hdr);
    out.extend_from_slice(&new_code);
    out.push(0x00);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个只包含目标区域的假文件，区域内容用 1..=334 填充
    fn fake_file() -> Vec<u8> {
        let mut file = vec![0u8; REGION_OFF + REGION_LEN];
        for (i, b) in file[REGION_OFF..].iter_mut().enumerate() {
            *b = (i % 256) as u8;
        }
        file
    }

    #[test]
    fn region_hash_is_sha256_hex() {
        let h = region_hash(&fake_file());
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn patch_keeps_region_size_and_header() {
        let file = fake_file();
        let out = build_region(&file);
        let region = &file[REGION_OFF..];
        assert_eq!(out.len(), REGION_LEN);
        // fat header 只有 code_size 被改成 321
        assert_eq!(&out[..4], &region[..4]);
        assert_eq!(&out[4..8], &[0x41, 0x01, 0x00, 0x00]);
        assert_eq!(&out[8..12], &region[8..12]);
        // 未改动字节应与原区域一一对应（新 code 相对原 code 少 1 字节 + 5 字节守卫）
        for &(new_start, new_end, old_start) in &[
            (0x00, 0x14, 0x00),
            (0x18, 0x3A, 0x18),
            (0x3E, 0x8C, 0x3E),
            (0x8C, 0x97, 0x92),
            (0x98, 0xAB, 0x9E),
            (0xB1, 0xD8, 0xB2),
            (0xD9, 0xE5, 0xDA),
            (0xE9, 321, 0xEA),
        ] {
            assert_eq!(
                &out[12 + new_start..12 + new_end],
                &region[12 + old_start..12 + old_start + (new_end - new_start)]
            );
        }
        assert_eq!(&out[12 + 0xAC..12 + 0xB1], &[0x11, 0x06, 0x16, 0x31, 0x1A]);
        // 末尾填充
        assert_eq!(out[REGION_LEN - 1], 0x00);
    }

    #[test]
    fn patch_fixes_branch_offsets() {
        let out = build_region(&fake_file());
        let code = &out[12..12 + 321];
        assert_eq!(&code[0x14..0x18], &[0x20, 0x01, 0x00, 0x00]);
        assert_eq!(&code[0x3A..0x3E], &[0xA1, 0x00, 0x00, 0x00]);
        assert_eq!(code[0x97], 0x39);
        assert_eq!(code[0xAB], 0x1F);
        assert_eq!(code[0xB0], 0x1A);
        assert_eq!(code[0xD8], 0xBF);
        assert_eq!(&code[0xE5..0xE9], &[0x55, 0xFF, 0xFF, 0xFF]);
    }
}
