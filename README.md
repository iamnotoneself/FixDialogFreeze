# The Darkside Detective — 对话框卡死一键修复工具

修复《黑暗侦探》（The Darkside Detective）游戏中**对话框随机卡死、按任何键都没反应**的问题。

双击运行即可，无需安装任何依赖（仅使用 Windows 自带的 cmd 和 PowerShell）。

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
`ArgumentOutOfRangeException`，协程被异常杀死，对话框停留在"异步格式化中"状态
且永远无法退出 —— 于是按哪里都没反应。

## 修复方式

- 在 `Insert` 前加入边界保护：`startCharIdx <= 0` 时跳过插入
- 同时删除一段无用的死代码腾出空间，**就地改写方法体**
- 文件总长度不变，不影响 DLL 中其它任何代码
- 修复前自动备份为 `SpookyDoorway.DarksideShared.dll.bak`
- 写入后立即校验，校验不通过会自动还原备份

## 使用方法（三选一）

1. **双击运行** `FixDialogFreeze.bat` —— 自动从 Epic 游戏清单 / 注册表 / 硬盘搜索定位游戏
2. **把 DLL 拖到 bat 文件上** —— `SpookyDoorway.DarksideShared.dll` 或整个游戏文件夹都可以
3. **把游戏文件夹作为参数运行**

```
FixDialogFreeze.bat "D:\GAME\TheDarksideDetectivehsD4J"
```

运行结果：

- `[成功] 修复完成！` —— 可以启动游戏验证了
- `[信息] 该文件已经打过补丁` —— 无需重复修复（此时输入 `R` + 回车可还原原版）
- `[失败] 无法识别的文件版本` —— 游戏可能已更新，工具**不会做任何修改**，请提交 Issue

## 注意事项

- 若 Epic 的"验证 / 修复"还原本文件，重新运行本工具即可
- 本工具只修改 `SpookyDoorway.DarksideShared.dll` 中已验证的 334 字节区域，其它内容一律不动
- 不兼容或未知版本的文件会被安全拒绝，绝不会写入未验证的数据

## 支持的版本

| 项目 | 状态 |
|------|------|
| 平台 | Epic Games（已验证） |
| 文件 | `SpookyDoorway.DarksideShared.dll`（区域哈希见脚本内常量） |
| 系统 | Windows 10 / 11（依赖自带 PowerShell 5.1） |

其它平台（Steam / GOG）理论上同版本 DLL 适用，欢迎测试后反馈。

## 提交 Issue

遇到"无法识别的文件版本"或修复无效时，请提供：

- 游戏平台（Epic / Steam / GOG）与购买来源
- 游戏版本号
- 运行截图或完整输出文字

## 免责说明

本工具直接修改游戏安装目录中的 DLL 文件。虽然有自动备份和写入后校验，
仍建议重要存档另行备份。使用本工具即表示理解并接受相应风险。
