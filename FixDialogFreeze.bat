@echo off
chcp 65001 >nul
title The Darkside Detective - Dialog Freeze Fix
set "DDFIX_SRC=%~f0"
set "DDFIX_TARGET=%~1"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$c=Get-Content -LiteralPath $env:DDFIX_SRC -Encoding UTF8; $m=-1; for($j=0;$j -lt $c.Count;$j++){if($c[$j] -like '# POWERSHELL_BODY*'){$m=$j;break}}; if($m -lt 0){Write-Host '[ERROR] marker missing'; exit 1}; Set-Content -LiteralPath ($env:TEMP+'\ddfix_body.ps1') -Value $c[($m+1)..($c.Count-1)] -Encoding UTF8; & ($env:TEMP+'\ddfix_body.ps1'); exit $LASTEXITCODE"
set "RC=%ERRORLEVEL%"
del /q "%TEMP%\ddfix_body.ps1" 2>nul
echo.
pause
exit /b %RC%
# POWERSHELL_BODY
# ==============================================================
#  The Darkside Detective（黑暗侦探）对话框卡死 一键修复工具
#
#  Bug 原因：
#    游戏脚本 SpookyDoorway.DarksideShared.dll 中的协程
#    PunctuationStartRule 格式化台词时执行
#        lastString.Insert(startCharIdx - 1, "\n")
#    当台词首字符就是断行标点（如引号）时 startCharIdx = 0，
#    index = -1 抛出 ArgumentOutOfRangeException，协程死亡，
#    对话框停留在“异步格式化中”状态且永远无法退出 -> 按哪里都没反应。
#
#  修复方式：
#    在 Insert 前加入边界保护：startCharIdx <= 0 时跳过插入；
#    同时删除一段无用的死代码腾出空间。就地改写方法体，
#    文件总长度不变，不影响 DLL 中其它任何代码。
#    修复前会自动备份为 原文件名.dll.bak。
#
#  用法：
#    1) 直接双击运行（自动查找 Epic/Steam 安装的游戏）
#    2) 或把 SpookyDoorway.DarksideShared.dll（或整个游戏文件夹）
#       拖到本文件上运行
#    若 Epic 的“验证/修复”还原本文件，重新运行本工具即可。
# ==============================================================

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = 'Stop'

# —— 已验证的方法体区域（文件偏移 0x136A4，长 334 字节）——
$REGION_OFF    = 0x136A4
$REGION_LEN    = 334
$HASH_ORIGINAL = 'e4c78062dd4af2eb85db85943ebaa22c324bde12de9d1d3e08ee4fedeaee2720'
$HASH_PATCHED  = 'cebe52902487fb8e4b5e8b5a51c686e0728604ac1249cfbe20c49f82a5861914'
$REL           = 'thedarksidedetective_Data\Managed\SpookyDoorway.DarksideShared.dll'

function Get-RegionHash([byte[]]$file) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { $h = $sha.ComputeHash($file, $REGION_OFF, $REGION_LEN) }
    finally { $sha.Dispose() }
    return (($h | ForEach-Object { $_.ToString('x2') }) -join '')
}

# 把用户拖进来的路径解析成目标 DLL
function Resolve-Candidate([string]$t) {
    if (-not $t) { return $null }
    $t = $t.Trim().Trim('"')
    if (-not (Test-Path -LiteralPath $t)) { return $null }
    if (Test-Path -LiteralPath $t -PathType Leaf) { return (Resolve-Path -LiteralPath $t).Path }
    foreach ($c in @((Join-Path $t $REL),
                     (Join-Path $t 'SpookyDoorway.DarksideShared.dll'),
                     (Join-Path $t 'Managed\SpookyDoorway.DarksideShared.dll'))) {
        if (Test-Path -LiteralPath $c) { return $c }
    }
    return $null
}

# 自动查找：拖入参数 -> Epic 游戏清单 -> 已安装程序注册表 -> 硬盘两层搜索
function Find-GameDll {
    $r = Resolve-Candidate $env:DDFIX_TARGET
    if ($r) { return $r }

    $mf = Join-Path $env:ProgramData 'Epic\EpicGamesLauncher\Data\Manifests'
    if (Test-Path -LiteralPath $mf) {
        foreach ($f in (Get-ChildItem -LiteralPath $mf -Filter '*.item' -ErrorAction SilentlyContinue)) {
            $line = Get-Content -LiteralPath $f.FullName -ErrorAction SilentlyContinue |
                Where-Object { $_ -like 'InstallLocation=*' } | Select-Object -First 1
            if ($line) {
                $c = Join-Path ($line.Substring(16)) $REL
                if (Test-Path -LiteralPath $c) { return $c }
            }
        }
    }

    $reg = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
           'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
           'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
    foreach ($item in (Get-ItemProperty $reg -ErrorAction SilentlyContinue |
            Where-Object { $_.DisplayName -like '*Darkside*' -and $_.InstallLocation })) {
        $c = Join-Path $item.InstallLocation $REL
        if (Test-Path -LiteralPath $c) { return $c }
    }

    foreach ($d in [System.IO.DriveInfo]::GetDrives()) {
        if (-not ($d.IsReady -and $d.DriveType -eq [System.IO.DriveType]::Fixed)) { continue }
        try {
            $hits = Get-ChildItem -LiteralPath $d.RootDirectory.FullName -Directory -Depth 2 -Force `
                    -ErrorAction SilentlyContinue | Where-Object { $_.Name -like '*Darkside*' }
            foreach ($h in $hits) {
                $c = Join-Path $h.FullName $REL
                if (Test-Path -LiteralPath $c) { return $c }
            }
        } catch { }
    }
    return $null
}

# 就地改写方法体：加守卫 + 删死代码 + 修正分支偏移
function Invoke-Patch([byte[]]$file) {
    $hdr  = [byte[]]($file[$REGION_OFF..($REGION_OFF + 11)])
    $code = [byte[]]($file[($REGION_OFF + 12)..($REGION_OFF + $REGION_LEN - 1)])   # 322 字节
    if ($code.Length -ne 322) { throw "内部错误：code 长度 $($code.Length)" }

    $guard = [byte[]](0x11, 0x06, 0x16, 0x31, 0x1A)   # ldloc.s 6; ldc.i4.0; ble.s skip
    # 删除死代码 [0x8C,0x92)，在 0xAC 处插入守卫
    $new = [byte[]]($code[0..0x8B] + $code[0x92..0xB1] + $guard + $code[0xB2..0x141])
    if ($new.Length -ne 321) { throw "内部错误：新 code 长度 $($new.Length)" }

    # —— 修正因位移而变化的分支偏移 ——
    $new[0x14] = 0x20; $new[0x15] = 0x01; $new[0x16] = 0x00; $new[0x17] = 0x00   # beq    +0x120
    $new[0x3A] = 0xA1; $new[0x3B] = 0x00; $new[0x3C] = 0x00; $new[0x3D] = 0x00   # br     +0xA1
    $new[0x97] = 0x39                                                             # br.s
    $new[0xAB] = 0x1F                                                             # bne.un.s
    $new[0xB0] = 0x1A                                                             # ble.s（守卫）
    $new[0xD8] = 0xBF                                                             # blt.s
    $new[0xE5] = 0x55; $new[0xE6] = 0xFF; $new[0xE7] = 0xFF; $new[0xE8] = 0xFF   # blt

    # —— fat header 中的 code_size：322 -> 321 ——
    $hdr[4] = 0x41; $hdr[5] = 0x01; $hdr[6] = 0x00; $hdr[7] = 0x00
    return [byte[]]($hdr + $new + [byte[]](0x00))
}

# ===================== 主流程 =====================
$dll = Find-GameDll
if (-not $dll) {
    Write-Host '[提示] 自动查找未找到游戏文件。' -ForegroundColor Yellow
    $p = Read-Host '请把 SpookyDoorway.DarksideShared.dll 或游戏文件夹拖到本窗口后回车'
    $dll = Resolve-Candidate $p
}
if (-not $dll) {
    Write-Host '[失败] 仍未找到 SpookyDoorway.DarksideShared.dll。' -ForegroundColor Red
    Write-Host '请打开游戏安装目录，把该 DLL 或整个游戏文件夹拖到本 .bat 文件上后重试。'
    exit 1
}
Write-Host "目标文件: $dll"

try { $bytes = [System.IO.File]::ReadAllBytes($dll) }
catch { Write-Host "[失败] 无法读取文件：$($_.Exception.Message)" -ForegroundColor Red; exit 1 }

if ($bytes.Length -lt ($REGION_OFF + $REGION_LEN)) {
    Write-Host '[失败] 文件长度异常，可能不是预期的游戏版本，未做任何修改。' -ForegroundColor Red
    exit 2
}
$hash = Get-RegionHash $bytes
$bak  = $dll + '.bak'

# —— 已修复过 ——
if ($hash -eq $HASH_PATCHED) {
    Write-Host '[信息] 该文件已经打过补丁，无需重复修复。' -ForegroundColor Cyan
    if (Test-Path -LiteralPath $bak) {
        try {
            if ((Get-RegionHash ([System.IO.File]::ReadAllBytes($bak))) -eq $HASH_ORIGINAL) {
                $r = Read-Host '输入 R 还原为原版（直接回车退出）'
                if ($r -ieq 'R') {
                    [System.IO.File]::WriteAllBytes($dll, [System.IO.File]::ReadAllBytes($bak))
                    Write-Host '[成功] 已还原为原版。' -ForegroundColor Green
                }
            }
        } catch { Write-Host "[失败] 还原出错：$($_.Exception.Message)" -ForegroundColor Red }
    }
    exit 0
}

# —— 无法识别的版本：安全退出 ——
if ($hash -ne $HASH_ORIGINAL) {
    Write-Host '[失败] 无法识别的文件版本（游戏可能已更新，或与本工具适配的版本不一致）。' -ForegroundColor Red
    Write-Host '为安全起见未做任何修改。请到本工具的 GitHub 仓库提交 Issue，'
    Write-Host '注明游戏平台（Epic / Steam 等）与游戏版本。'
    exit 2
}

# —— 备份 ——
try {
    $ok = $false
    if (Test-Path -LiteralPath $bak) {
        $ok = ((Get-RegionHash ([System.IO.File]::ReadAllBytes($bak))) -eq $HASH_ORIGINAL)
    }
    if (-not $ok) { Copy-Item -LiteralPath $dll -Destination $bak -Force }
} catch {
    Write-Host "[失败] 备份失败：$($_.Exception.Message)" -ForegroundColor Red
    exit 3
}

# —— 打补丁 ——
try {
    $newRegion = Invoke-Patch $bytes
    if ($newRegion.Length -ne $REGION_LEN) { throw "内部错误：新区域长度 $($newRegion.Length)" }
    [Array]::Copy($newRegion, 0, $bytes, $REGION_OFF, $REGION_LEN)
    [System.IO.File]::WriteAllBytes($dll, $bytes)
} catch [System.IO.IOException] {
    Write-Host '[失败] 无法写入文件，游戏可能正在运行，请先关闭游戏后重试。' -ForegroundColor Red
    exit 3
} catch {
    Write-Host "[失败] $($_.Exception.Message)" -ForegroundColor Red
    exit 4
}

# —— 写入后校验 ——
try {
    $verify = Get-RegionHash ([System.IO.File]::ReadAllBytes($dll))
    if ($verify -ne $HASH_PATCHED) {
        if (Test-Path -LiteralPath $bak) {
            Copy-Item -LiteralPath $bak -Destination $dll -Force
            Write-Host '[失败] 写入后校验不通过，已自动还原备份，未做任何改动。' -ForegroundColor Red
        } else {
            Write-Host '[失败] 写入后校验不通过！请手动用备份还原。' -ForegroundColor Red
        }
        exit 5
    }
} catch {
    Write-Host "[失败] 校验出错：$($_.Exception.Message)" -ForegroundColor Red
    exit 5
}

Write-Host '[成功] 修复完成！' -ForegroundColor Green
Write-Host "  已修改: $dll"
Write-Host "  原文件备份: $bak"
Write-Host '  现在可以启动游戏验证，之前卡住的对话场景不会再出现。'
Write-Host '  提示：若 Epic 的“验证/修复”还原本文件，重新运行本工具即可。'
exit 0
