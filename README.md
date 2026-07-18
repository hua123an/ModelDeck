# ModelDeck

ModelDeck 是一个本地运行的 LLM 服务商聚合管理工具。它管理服务商、检测模型、验证实际请求并查询余额，但不充当 API 网关，也不转发业务请求。

## 功能

- 管理 New API、Sub2API 和 OpenAI Compatible 服务商
- API Key 保存到操作系统钥匙串，普通配置保存在 Tauri 应用数据目录
- 通过 `/v1/models` 汇总模型
- 自动尝试 `/v1/responses` 与 `/v1/chat/completions`
- 单模型和批量最小请求测试，记录状态码、延迟、错误与检测时间
- 按服务商类型尝试余额接口，并严格校验余额字段
- 统一账户中心：余额、消费、请求数、订阅、到期时间和分组倍率
- 跨服务商 API Key 管理：创建、编辑、启停、删除、额度、过期时间和切换分组
- New API 系统 Access Token + 用户 ID，Sub2API JWT + 可选 Refresh Token 自动刷新
- 使用统计：按服务商汇总请求量、Token 与实际消费
- 系统配置档案：一键切换 Codex `config.toml` 或 Claude Code `settings.json`
- 最小字段合并，保留 Codex 登录态、项目设置、历史记录和未知配置字段
- 每次切换自动备份并支持一键恢复，使用原子写入并保持文件权限
- 导出 pi 使用的 `models.json`，密钥使用环境变量占位符

## 开发

```bash
npm install
npm run tauri dev
```

仅运行浏览器预览：

```bash
npm run dev
```

浏览器预览使用本地演示数据；真实网络请求、持久化和钥匙串只在 Tauri 桌面应用中启用。

## 验证

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```
