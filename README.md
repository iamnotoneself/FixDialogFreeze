# The Darkside Detective — 对话框卡死一键修复工具

修复《黑暗侦探》（The Darkside Detective）游戏中**对话框随机卡死、按任何键都没反应**的问题。

2.0 版用 Rust 重写：单个 exe，无需安装任何运行库，双击即可运行。

## 症状

- 对话进行到某一句时，对话框停住不再响应
- 键盘、鼠标点击均无反应，只能强杀游戏进程
- 问题随机出现，与具体存档/平台无关（Epic 版已确认）

## 原因

游戏脚本 `SpookyDoorway.DarksideShared.dll` 中的协程 `PunctuationStartRule` 在格式化台词时执行：

```csharp
text.Insert(startCharIdx - 1, "\n")
```

当台词首字符就是断行标点（如引号）时 `startCharIdx = 0`，`Insert(-1, ...)` 抛出
`ArgumentOutOfRangeException`，协程被异常杀死，对话框停留在“异步格式化中”状态
且永远无法退出 —— 于是按哪里都没反应。

## 修复方式

- 在 `Insert` 前加入边界保护：`startCharIdx <= 0` 时跳过插入
- 同时删除一段无用的死代码腾出空间，**就地改写方法体**
- 文件总长度不变，不影响 DLL 中其它任何代码
- 修复前自动备份为 `SpookyDoorway.DarksideShared.dll.bak`
- 写入后立即校验，校验不通过会自动还原备份

定位方式是「固定文件偏移 0x136A4 + 334 字节区域 SHA-256 校验」：

| 状态 | 区域哈希（前 8 位） |
| --- | --- |
| 原版（有 bug） | `e4c78062…` |
| 已修复 | `cebe5290…` |

只要区域哈希不是这两个值，工具会直接拒绝修改，避免误伤其它版本。

## 使用方法（三选一）

1. **双击运行** `FixDialogFreeze.exe` —— 自动从 Epic 游戏清单 / 注册表 / 硬盘搜索定位游戏
2. **把 DLL 拖到 exe 上** —— `SpookyDoorway.DarksideShared.dll` 或整个游戏文件夹都可以
3. **把游戏文件夹作为参数运行**

```
FixDialogFreeze.exe "D:\Games\TheDarksideDetective"
```

已经打过补丁的文件会提示“无需重复修复”，并允许输入 `R` 从 `.bak` 还原为原版。

## 从源码构建

需要 Rust 工具链（stable）：

```
cargo build --release
```

产物位于 `target/release/FixDialogFreeze.exe`。

## 说明

若 Epic 的“验证/修复”把文件还原成原版，重新运行本工具即可。
遇到无法识别的版本，请到本仓库提交 Issue，注明游戏平台与游戏版本。
