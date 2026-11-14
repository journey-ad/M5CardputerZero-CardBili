# CI 与发布

## 自动检查

`.github/workflows/ci.yml` 在推送 `main`、提交 Pull Request、推送 `v*` 标签及手动触发时运行

- Linux 桌面配置编译检查
- Shell 脚本检查、Python 语法检查与工作流校验
- Gitleaks 扫描完整 Git 历史，报告隐藏凭据内容
- ARM64 Linux runner 构建 musl 应用及精简 FFmpeg
- 打包设备产物、对应源码与 SHA-256 校验文件

普通检查与构建任务使用只读仓库权限。PR 构建使用公开依赖与空值配置，设备和登录凭据留在本地

## 发布版本

1. 更新 `cardbili/Cargo.toml` 版本并同步 `Cargo.lock`
2. 完成设备安装、播放、导航和登录验证，确认截图与说明适合公开
3. 提交后创建对应标签，例如 `v0.1.0`，再推送分支与标签
4. CI 验证标签与 Cargo 版本一致，生成 GitHub Release 草稿
5. 检查草稿附件、变更说明及许可证后，在 GitHub 发布草稿

发布任务使用 GitHub 自动提供的 `GITHUB_TOKEN`，只有该任务获得 `contents: write` 权限。失败或重跑时可在 Actions 查看构建产物；已存在的同名 Release 保持原状，任务报错供维护者处理

## 发布附件

| 文件 | 内容 |
| --- | --- |
| `cardbili-<版本>-aarch64-linux-musl.tar.gz` | 应用、解码器、安装脚本、图标、配置模板和许可证 |
| `cardbili-<版本>-source.tar.gz` | 项目源码、Cargo vendored 依赖、FFmpeg 修改后的源码及构建记录 |
| `SHA256SUMS` | 两个归档的 SHA-256 |

设备二进制含 Slint GPLv3 组件，发布时将对应源码包一起提供。源码包保留项目自身的 MIT 许可及每个依赖的许可证，详情见 [第三方许可证](../THIRD_PARTY_NOTICES.md)

源码包的 `cardbili/.cargo/config.toml` 已指向其中的 `vendor/`。在 `cardbili/` 下执行 Cargo 命令可使用这些依赖。FFmpeg 修改后的源码位于 `ffmpeg-source.tar.xz`，解压后按照 `cardbili/ffmpeg/Dockerfile` 中的 configure 选项编译；`cardbili/ffmpeg/CHANGES.md` 记录修改内容。构建工具链与系统开发包仍由构建环境提供

## 本地预演

```sh
bash scripts/build-device.sh
python3 scripts/package-release.py
cd dist
shasum -a 256 -c SHA256SUMS
```

打包脚本读取当前 Git 索引中的项目文件，使用工作区内容生成源码包。新文件需先加入索引，私有 `.secrets` 文件、缓存和构建目录按路径排除
