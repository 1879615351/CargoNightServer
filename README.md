# CargoNight Server

在线桌游平台后端服务，基于 **Rust + Axum + PostgreSQL** 构建，为桌面端和 Android 端提供 REST API 与 WebSocket 实时通信。

## 技术栈

- **框架**: Axum 0.8 (HTTP + WebSocket)
- **异步运行时**: Tokio
- **数据库**: PostgreSQL 16 + SQLx 0.8 (连接池 + 自动迁移)
- **认证**: JWT (jsonwebtoken) + Argon2 密码哈希
- **实时通信**: WebSocket (tokio-tungstenite) + WebRTC 信令
- **日志**: Tracing + tracing-subscriber
- **序列化**: Serde + Serde JSON
- **UUID**: uuid v4

## API 模块

| 模块 | 端点 | 描述 |
|------|------|------|
| `/api/auth` | register, login, me | 用户注册/登录/个人信息 |
| `/api/home` | stats | 首页聚合数据 (在线人数、热门游戏/房间、好友、公告) |
| `/api/games` | list | 游戏列表 (含在线人数、房间数、标签) |
| `/api/rooms` | create, join, leave, ready, start | 房间全生命周期管理 |
| `/api/chat` | send, get | 房间聊天消息 |
| `/api/avalon` | start, state, select-team, team-vote, mission-vote, end-speaking, assassinate, confirm-settlement, disconnect | 阿瓦隆游戏完整流程 API |
| `/api/ai` | add | AI 玩家管理 |
| `/api/profile` | records, stats | 用户游戏记录与统计 |
| `/api/friends` | search, add, accept, remove | 好友系统 |
| `/ws` | connect | WebSocket 实时通信 (聊天/房间事件/阿瓦隆状态推送/WebRTC 信令) |

## 阿瓦隆游戏引擎

### 阶段流程
```
RoleReveal → Proposal → Discussion → Vote → Mission → Result
    ↑                                                      │
    └──────────── (循环至 3 次成功/失败 或 第 5 轮) ←────────┘
                              ↓
                       Assassination → End
```

### 角色视野规则
| 角色 | 阵营 | 夜间视野 |
|------|------|----------|
| 梅林 (Merlin) | 好人 | 能看到莫甘娜、刺客、爪牙、莫德雷德 (除了奥伯伦) |
| 派西维尔 (Percival) | 好人 | 能看到梅林和莫甘娜 (分不清谁是谁) |
| 忠臣 (Loyal Servant) | 好人 | 无特殊视野 |
| 刺客 (Assassin) | 坏人 | 能看到所有坏人 (除了奥伯伦) |
| 莫甘娜 (Morgana) | 坏人 | 能看到所有坏人 (除了奥伯伦)，伪装成梅林 |
| 爪牙 (Minion) | 坏人 | 能看到所有坏人 (除了奥伯伦) |
| 莫德雷德 (Mordred) | 坏人 | 能看到所有坏人 (除了奥伯伦)，梅林看不到他 |
| 奥伯伦 (Oberon) | 坏人 | 看不到其他坏人，其他坏人也看不到他 |

## 快速开始

### 前置条件

- Rust 1.80+
- PostgreSQL 16

### 配置

创建 `.env` 文件或设置环境变量：

```env
DATABASE_URL=postgres://postgres@localhost:5432/cargonight
JWT_SECRET=cargonight-dev-jwt-secret-key-2026
SERVER_PORT=8080
SERVER_HOST=0.0.0.0
RUST_LOG=info
```

- `SERVER_HOST=0.0.0.0` 监听所有网络接口，局域网内其他设备可访问
- `RUST_LOG=info` 控制日志级别 (可选 `debug`, `trace`)

### 本地开发

```bash
# 启动 PostgreSQL (Windows)
"C:/Program Files/PostgreSQL/16/bin/pg_ctl.exe" start -D "C:/Program Files/PostgreSQL/16/data"

# 启动服务 (开发模式)
cargo run

# 生产构建
cargo build --release
./target/release/cargo-night-server.exe
```

服务默认运行在 `http://0.0.0.0:8080`。

### Docker 部署

```bash
docker compose up -d    # 自动启动 PostgreSQL + Server
```

## 数据库迁移

迁移文件位于 `migrations/` 目录，服务启动时自动执行 (SQLx migrate)：

| 文件 | 内容 |
|------|------|
| `20260505000000_init.sql` | 用户表、房间表、游戏表、聊天记录表 |
| `20260506000000_game_records.sql` | 游戏记录表 (含轮次详情 JSON) |
| `20260506000001_friends.sql` | 好友关系表 (待确认/已接受) |
| `20260506000002_short_ids.sql` | 房间短 ID 索引 |

## 项目结构

```
src/
├── main.rs         # 入口 (Tracing 初始化 → 配置加载 → DB 连接 → 路由注册 → 启动)
├── config.rs       # 环境变量配置 (dotenvy)
├── db.rs           # PgPool 连接池 + AppState (游戏/AI/在线状态)
├── error.rs        # 统一错误类型与 HTTP 响应
├── handlers/       # HTTP 路由处理
│   ├── auth.rs     # 注册/登录/JWT 签发
│   ├── home.rs     # 首页聚合统计
│   ├── games.rs    # 游戏列表
│   ├── rooms.rs    # 房间 CRUD + 准备/开始
│   ├── chat.rs     # 聊天消息
│   ├── avalon.rs   # 阿瓦隆完整 API
│   ├── ai.rs       # AI 添加/移除
│   ├── profile.rs  # 用户资料 + 游戏记录
│   └── friends.rs  # 好友搜索/添加/接受/删除
├── middleware/      # 中间件
│   └── auth.rs     # JWT 认证 (Bearer Token → UserId 提取)
├── models/         # 数据库模型
│   ├── user.rs     # 用户
│   ├── room.rs     # 房间 + 玩家
│   ├── game.rs     # 游戏定义
│   ├── chat.rs     # 聊天记录
│   ├── friend.rs   # 好友关系
│   └── game_record.rs # 游戏记录
├── ws/             # WebSocket 子系统
│   ├── handler.rs   # WS 握手 + 消息路由
│   ├── manager.rs   # 房间连接池 + 事件广播
│   └── signaling.rs # WebRTC SDP/ICE 信令转发
└── game/
    └── avalon/      # 阿瓦隆游戏引擎
        ├── engine.rs # 状态机 (阶段推进/队伍审批/任务计票/刺杀判定)
        ├── roles.rs  # 角色定义 + 视野矩阵 + 失败票数表
        └── ai.rs     # AI 决策 (队伍选择/投票/任务成败/发言生成)
```

## 许可

MIT
