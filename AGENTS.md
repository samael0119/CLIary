# Codebase Knowledge Graph

代码发现优先使用 codebase-memory-mcp：`search_graph`、`trace_path`、`get_code_snippet`、`query_graph`、`get_architecture`。项目未索引时先运行 `index_repository`。搜索字符串、配置、非代码文件或图工具结果不足时可回退到文本搜索和文件读取。

# 文档与开发记录

- 遵循 [docs/README.md](docs/README.md) 的文档分类与命名约定。
- 实施计划、评审、每日改进／会话记录、个人自动化说明和临时原型统一写入被忽略的 `docs/dev/` 相应子目录。
- 有日期的记录使用 `YYYY-MM-DD-<topic>.md`，主题使用小写英文 kebab-case；长期复用的本地说明使用 `<topic>.md`。
- 不再向旧的 `docs/plans/`、`docs/reviews/`、`docs/improvement-runs/`、`docs/superpowers/` 写入新记录，也不使用 `git add -f` 提交本地开发记录。
- 可共享的稳定结论整理进正式项目文档；正式文档不依赖本地开发记录，不包含个人邮箱、本机绝对路径或敏感凭据。

# Response Language

最终总结用中文回答。
