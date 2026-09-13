# Are You Healthy

基于 Rust 的健康检查 Web 服务：在 UI 中配置 HTTP 探测项与飞书机器人告警，后台定时检测可达性，数据保存在 SQLite。

## 功能

- 增删改查健康检查（URL、方法、期望状态码、间隔、启用/停用）
- 响应体检查点：包含、相等、不包含、正则匹配 / 正则不匹配
- 飞书自定义机器人 Webhook 告警（首次故障、冷却期内重复提醒、恢复通知）
- PushPlus（推送加）Token 告警，与飞书可并行启用
- 每 5 秒调度一次，按各条目 `interval_secs` 执行探测
- 每次检测写入历史记录，可按条目分页查询
- 后台定时清理过期/超量历史（可配置）
- 静态管理界面（`rust_web_app/assets`）

## 运行

```bash
cd rust_web_app
cargo run
```

浏览器打开 [http://localhost:8080](http://localhost:8080)。

### 配置文件

在 `rust_web_app/config.toml` 中修改服务参数（不存在时使用内置默认值）。主要项：

| 配置段 | 字段 | 默认值 | 说明 |
|--------|------|--------|------|
| `server` | `host` / `port` | `0.0.0.0` / `8080` | HTTP 监听 |
| `database` | `path` | `data/health.db` | SQLite 路径 |
| `assets` | `dir` | `assets` | 静态资源目录 |
| `history` | `retention_days` | `30` | 历史保留天数，`0` 不按时间删 |
| `history` | `max_per_check` | `1000` | 每条最多保留条数，`0` 不限制 |
| `history` | `cleanup_interval_secs` | `3600` | 清理任务间隔（秒） |
| `probe` | `capture_max_bytes` | `8192` | 响应体写入上限（字节） |
| `scheduler` | `tick_secs` | `5` | 调度扫描间隔（秒） |
| `http_client` | `timeout_secs` | `15` | 探测 HTTP 超时 |
| `log` | `level` | `info` | 日志级别（可被 `RUST_LOG` 覆盖） |

## 飞书配置

1. 在飞书群 → 设置 → 群机器人 → 添加「自定义机器人」
2. 复制 Webhook URL 到管理页「飞书告警」
3. 勾选「启用飞书告警」并保存

## PushPlus 配置

1. 打开 [pushplus 一对一消息](https://www.pushplus.plus/push1.html) 并登录
2. 复制**用户 Token** 到管理页「PushPlus 告警」（需完成实名认证方可调用发送接口）
3. 勾选「启用 PushPlus 告警」并保存；可用「发送测试消息」验证

## API 摘要

- `GET/POST /api/checks` — 列表 / 创建
- `GET/PUT/DELETE /api/checks/:id` — 详情 / 更新 / 删除（创建/更新 body 可带 `checkpoints` 数组）
- `GET/PUT /api/checks/:id/checkpoints` — 检查点列表 / 全量替换
- `POST /api/checks/:id/run` — 立即执行一次检测（含告警逻辑）
- `GET /api/checks/:id/history?limit=&offset=` — 检测历史（含 `request_message` / `response_message` 报文）
- `GET/PUT /api/feishu` — 飞书配置
- `POST /api/feishu/test` — 发送测试告警（body 可选 `webhook_url`，默认用已保存配置）
- `GET/PUT /api/pushplus` — PushPlus 配置（`token`、`enabled`、`alert_cooldown_secs`）
- `POST /api/pushplus/test` — 测试 PushPlus（body 可选 `token`）
