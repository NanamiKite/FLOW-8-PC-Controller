# FLOW 8 PC Controller

[English](README.md)

FLOW 8 PC Controller 是一款使用 Rust 编写的非官方 Behringer FLOW 8 桌面控制器。它通过蓝牙显示调音台回报的状态，并提供混音、路由、效果器和设备快照控制。

本项目与 Behringer、Music Tribe 无关联。

## 当前状态

项目仍在开发。在已测试的 Windows／DirectHCI 环境中，真机日志显示程序完成了初始状态同步、进入 Ready，并记录到控制写入和设备通知。这不代表所有 Windows 蓝牙控制器或 FLOW 8 固件均已兼容。Linux 的 btleplug 后端已经存在，但这里尚无 Linux 真机验收记录。具体证据与待验证项见[硬件验证记录](docs/hardware-validation.md)。

未连接时仍可查看混音界面；只有完成设备状态同步后，设备控制才会启用。

## Windows 安装

Windows x64 安装包将作为版本 Release 附件发布。

FLOW 8 安装包只安装图形程序。Windows 蓝牙控制还需要单独安装 [DirectHCI](https://github.com/NanamiKite/DirectHCI)：连接前须在 DirectHCI Control Panel 中启动服务、选择受支持的控制器；如果面板显示 **Not prepared**，请按照 DirectHCI 的说明准备控制器。FLOW 8 安装包不会代为完成这些操作。当前 DirectHCI SDK 获取控制器会话时还要求以管理员权限运行 FLOW 8 程序。详见 [Windows 安装说明](docs/installation.md)。

首次使用新的客户端身份连接时，请先在调音台上启用 **PAIR REMOTE / PAIR APP**，再在程序中连接。程序会在 `%LOCALAPPDATA%\FLOW 8 PC Controller\client-id.txt` 创建并复用客户端身份。请妥善保管此文件；删除后会生成新身份，可能需要重新配对。常规日志位于 `%LOCALAPPDATA%\FLOW 8 PC Controller\logs`。

## 从源码运行

需要 Rust 1.95 或更新版本。工作区目前通过本地路径依赖独立的 DirectHCI Rust SDK，因此两个仓库需要并列放置：

```text
parent/
├── FLOW 8 PC Controller/
└── DirectHCI/
```

在本仓库目录运行：

```sh
cargo run -p flow8-gui
```

Windows 需要另外安装并运行 DirectHCI 服务；Linux 通过 BlueZ／btleplug 使用系统蓝牙。如果源码位于 VMware 共享文件夹，请将 Cargo 构建产物放在本机文件系统。平台配置、构建和验证步骤见[开发说明](docs/DEVELOPMENT.md)。

## 文档

- [Windows 安装与前置要求](docs/installation.md)
- [开发说明](docs/DEVELOPMENT.md)
- [架构](docs/architecture.md)
- [协议实现](docs/protocol.md)
- [BLE 传输](docs/ble.md)
- [硬件验证记录](docs/hardware-validation.md)

旧版 C++／Qt 实现保存在独立的 `c++` 分支；当前分支为 Rust 应用。

## 许可证

FLOW 8 PC Controller 采用 GPL-3.0-only 许可证，详见 [LICENSE.txt](LICENSE.txt)。DirectHCI 是独立项目，其发布和依赖许可声明以该项目为准。
