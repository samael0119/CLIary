# 文档约定

仓库文档保留长期有效、可共享的产品与项目知识。开发过程中的记录统一放在本地 `docs/dev/`，该目录由 `.gitignore` 忽略。

## 随项目提交的文档

| 位置 | 用途 |
| --- | --- |
| [根目录 README](../README.md) | 使用、安装、构建、贡献与功能限制 |
| [PRODUCT.md](../PRODUCT.md) | 当前产品范围与约束 |
| [CLIary_PRD.md](CLIary_PRD.md) | 产品需求说明 |
| `docs/releases/` | 随版本发布的安装说明、变更与已知限制 |
| [screenshots/](screenshots/README.md) | 用于 README 的公开演示素材与复现方法，仅使用合成数据 |
| `docs/` 中其他正式文档 | 可共享的架构、接口、维护说明；使用小写英文 kebab-case 文件名，如 `architecture.md` |

正式文档描述当前约定和实际行为，不记录个人邮箱、本机绝对路径、会话过程或临时执行结果。已稳定且需要团队共享的结论，应整理为正式文档。

## 本地开发记录

统一使用以下目录，不再新增旧的 `docs/plans/`、`docs/reviews/`、`docs/improvement-runs/` 或按工具名划分的开发记录目录。

```text
docs/dev/
  plans/        # 实施计划、需求评估与提案
  reviews/      # 代码／设计评审、交付与验证记录
  runs/         # 每日改进、调试与会话记录
  automation/   # 个人自动化说明
  artifacts/    # 临时原型、预览、截图与执行输出
```

- 有日期的记录使用 `YYYY-MM-DD-<topic>.md`，例如 `2026-10-09-web-polish.md`；同日同主题可加 `-01`、`-02`。
- 主题使用小写英文 kebab-case；长期复用的本地说明使用 `<topic>.md`，例如 `automation/daily-improvement.md`。
- 临时素材按同样的日期与主题组织，例如 `artifacts/2026-09-29-ui-redesign-preview.html`；一组素材可放进 `artifacts/YYYY-MM-DD-<topic>/`。
- 生成记录的脚本、技能和自动化应使用这些位置；不要用 `git add -f` 提交本地开发记录。
- 已有 `.impeccable/review/` 是工具生成的本地 QA 缓存，继续忽略；正式演示素材放在 `docs/screenshots/`。
- 正式文档只能链接到随项目提交的内容，不依赖被忽略的开发记录。

`.gitignore` 不会自动停止跟踪已提交的文件，需要同时从 Git 索引移除并保留本地文件。此规则仅影响后续提交，已有提交历史中的内容仍然存在。
