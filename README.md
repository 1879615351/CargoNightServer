# CargoNight Server

在线桌游平台后端服务，基于 **Rust + Axum + PostgreSQL** 构建。

## 技术栈

- **框架**: Axum 0.8
- **异步运行时**: Tokio
- **数据库**: PostgreSQL 16 + SQLx 0.8
- **认证**: JWT + Argon2 密码哈希
- **实时通信**: WebSocket + WebRTC 信令
- **日志**: Tracing

## API 模块

| 模块 | 描述 |
|------|------|
| `/api/auth` | 用户注册、登录、Token 刷新 |
| `/api/home` | 首页数据 (在线人数、热门游戏等) |
| `/api/games` | 游戏列表 |
| `/api/rooms` | 房间创建、加入、管理 |
| `/api/chat` | 聊天消息 |
| `/api/avalon` | 阿瓦隆游戏逻辑 (组队、投票、任务、刺杀) |
| `/api/ai` | AI 玩家管理 |
| `/api/profile` | 用户资料与游戏记录 |
| `/api/friends` | 好友系统 (搜索、添加、删除) |
| `/ws` | WebSocket 实时通信 |

## 快速开始

### 前置条件

- Rust 1.80+
- PostgreSQL 16

### 配置

创建 `.env` 文件或设置环境变量：

```env
DATABASE_URL=postgres://postgres@localhost:5432/cargonight
JWT_SECRET=your-jwt-secret
SERVER_PORT=8080
SERVER_HOST=0.0.0.0
```

- `SERVER_HOST=0.0.0.0` 监听所有网络接口，局域网内其他设备可访问
- `RUST_LOG=info` 控制日志级别 (可选 `debug`)

### 本地开发

```bash
# 启动 PostgreSQL (Windows)
"C:/Program Files/PostgreSQL/16/bin/pg_ctl.exe" start -D "C:/Program Files/PostgreSQL/16/data"

# 启动服务
cargo run
```

服务默认运行在 `http://0.0.0.0:8080`。

### 生产构建

```bash
cargo build --release
./target/release/cargo-night-server.exe   # Windows
./target/release/cargo-night-server       # Linux/macOS
```

### Docker 部署

```bash
docker compose up -d
```

## 数据库迁移

迁移文件位于 `migrations/` 目录，服务启动时自动执行：

- `20260505000000_init.sql` — 初始表结构 (用户、房间、游戏)
- `20260506000000_game_records.sql` — 游戏记录表
- `20260506000001_friends.sql` — 好友系统表
- `20260506000002_short_ids.sql` — 短 ID 支持

## 项目结构

```
src/
├── main.rs         # 入口 (初始化日志、数据库、路由)
├── config.rs       # 环境变量配置
├── db.rs           # 数据库连接池与共享状态
├── error.rs        # 统一错误处理
├── handlers/       # HTTP 路由处理
│   ├── auth.rs     # 注册/登录
│   ├── home.rs     # 首页统计
│   ├── games.rs    # 游戏列表
│   ├── rooms.rs    # 房间管理
│   ├── chat.rs     # 聊天
│   ├── avalon.rs   # 阿瓦隆 API
│   ├── ai.rs       # AI 管理
│   ├── profile.rs  # 用户资料
│   └── friends.rs  # 好友系统
├── middleware/      # 中间件
│   └── auth.rs     # JWT 认证中间件
├── models/         # 数据模型
│   ├── user.rs
│   ├── room.rs
│   ├── game.rs
│   ├── chat.rs
│   ├── friend.rs
│   └── game_record.rs
├── ws/             # WebSocket 管理
│   ├── handler.rs   # WS 路由与升级
│   ├── manager.rs   # 连接管理与房间事件广播
│   └── signaling.rs # WebRTC 信令
└── game/
    └── avalon/      # 阿瓦隆游戏引擎
        ├── engine.rs # 游戏状态机 (阶段推进、阵营判定)
        ├── roles.rs  # 角色定义与视野规则
        └── ai.rs     # AI 决策逻辑
```

## 许可

MIT
