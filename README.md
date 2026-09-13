# Are You Healthy

基于 Rust 的健康检查 Web 服务：在 UI 中配置 HTTP 探测项与飞书机器人告警，后台定时检测可达性，数据保存在 SQLite。

## 功能

- 增删改查健康检查（URL、方法、期望状态码、间隔、启用/停用）
- 飞书自定义机器人 Webhook 告警（首次故障、冷却期内重复提醒、恢复通知）
- 每 5 秒调度一次，按各条目 `interval_secs` 执行探测
- 静态管理界面（`rust_web_app/assets`）

## 运行

```bash
cd rust_web_app
cargo run
```

浏览器打开 [http://localhost:8080](http://localhost:8080)。

### 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `HOST` | `0.0.0.0` | 监听地址 |
| `PORT` | `8080` | 端口 |
| `DATABASE_PATH` | `data/health.db` | SQLite 文件路径 |
| `ASSETS_DIR` | `assets` | 静态资源目录 |
| `RUST_LOG` | `info` | 日志级别 |

## 飞书配置

1. 在飞书群 → 设置 → 群机器人 → 添加「自定义机器人」
2. 复制 Webhook URL 到管理页「飞书告警」
3. 勾选「启用飞书告警」并保存

## API 摘要

- `GET/POST /api/checks` — 列表 / 创建
- `GET/PUT/DELETE /api/checks/:id` — 详情 / 更新 / 删除
- `GET/PUT /api/feishu` — 飞书配置
- `POST /api/feishu/test` — 发送测试告警（body 可选 `webhook_url`，默认用已保存配置）
