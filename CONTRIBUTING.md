# Contributing / 贡献指南

欢迎提交 Bug、工具目录补充和有明确收益的改进。中文或英文均可；较大功能请先开 Issue 讨论。

## 开发与验证

CI 使用 Rust 1.94.1：

```sh
rustup toolchain install 1.94.1 --profile minimal
rustup component add rustfmt --toolchain 1.94.1
cargo +1.94.1 build --locked -p cliary-cli
cargo +1.94.1 fmt --all -- --check
cargo +1.94.1 test --workspace --locked
python3 scripts/test_ci_package.py
```

打包测试在 Python 3.10 需要 tomli，Python 3.11+ 可直接运行。Catalog 条目和校验方法见 [README](README.md#contribute-catalog-entries)。文档遵循 [docs/README.md](docs/README.md)。

## 提交问题与 PR

- Bug 请包含版本、平台、复现步骤、期望和实际行为；使用人工最小示例。
- 从最新 main 建立分支，一个 PR 处理一个问题，说明改动与相关检查结果。CI 不可用时明确注明。
- 不提交个人 Shell 历史、数据库、别名快照、凭据或私人年报；截图和日志先脱敏。测试及演示使用隔离工作区和人工数据。
- 贡献使用项目的 [MIT license](LICENSE)；第三方材料保留许可证和出处。讨论针对代码与行为，尊重参与者。
- 安全问题请按 [SECURITY.md](SECURITY.md) 私密报告。
