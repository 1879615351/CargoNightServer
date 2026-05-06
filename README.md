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
| `/api/home` | 首页数据 |
| `/api/games` | 游戏列表 |
| `/api/rooms` | 房间创建、加入、管理 |
| `/api/chat` | 聊天消息 |
| `/api/avalon` | 阿瓦隆游戏逻辑 |
| `/api/ai` | AI 玩家 |
| `/api/profile` | 用户资料 |
| `/api/friends` | 好友系统 |
| `/ws` | WebSocket 实时通信 |

## 快速开始

### Docker 部署 (推荐)

```bash
docker compose up -d
```

### 本地开发

#### 前置条件

- Rust 1.80+
- PostgreSQL 16

#### 配置环境变量

创建 `.env` 文件：

```env
DATABASE_URL=postgres://cargonight:cargonight_dev@localhost:5432/cargonight
JWT_SECRET=your-jwt-secret
SERVER_PORT=8080
SERVER_HOST=0.0.0.0
```

#### 安装与运行

```bash
# 数据库迁移 (Docker 方式)
docker compose up -d postgres

# 启动服务
cargo run
```

服务默认运行在 `http://localhost:8080`。

## 项目结构

```
src/
├── main.rs         # 入口
├── config.rs       # 配置
├── db.rs           # 数据库连接
├── error.rs        # 错误处理
├── handlers/       # HTTP 路由处理
├── middleware/      # 中间件 (JWT 认证)
├── models/         # 数据模型
├── ws/             # WebSocket 管理
│   ├── handler.rs   # WS 路由
│   ├── manager.rs   # 连接管理
│   └── signaling.rs # WebRTC 信令
└── game/
    └── avalon/      # 阿瓦隆游戏引擎
        ├── engine.rs # 游戏状态机
        ├── roles.rs  # 角色定义
        └── ai.rs     # AI 逻辑
```

## 许可

MIT
