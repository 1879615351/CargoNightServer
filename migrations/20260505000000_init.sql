CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(64) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    avatar VARCHAR(8) DEFAULT '🎮',
    bio TEXT DEFAULT '',
    total_games INT DEFAULT 0,
    win_rate REAL DEFAULT 0,
    favorite_game VARCHAR(64) DEFAULT '',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS games (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(128) NOT NULL,
    description TEXT DEFAULT '',
    min_players INT NOT NULL,
    max_players INT NOT NULL,
    duration_minutes INT DEFAULT 30,
    difficulty VARCHAR(16) DEFAULT '简单',
    tags TEXT[] DEFAULT '{}',
    icon VARCHAR(8) DEFAULT '🎲',
    online_count INT DEFAULT 0,
    room_count INT DEFAULT 0,
    hot BOOLEAN DEFAULT FALSE
);

INSERT INTO games (id, name, description, min_players, max_players, duration_minutes, difficulty, tags, icon, online_count, room_count, hot) VALUES
    ('avalon', '阿瓦隆', '隐藏身份与推理的经典桌游，玩家分为正义与邪恶两方，通过任务和投票决出胜负', 5, 10, 30, '中等', ARRAY['推理','身份隐藏','团队'], '🛡️', 245, 32, true),
    ('werewolf', '狼人杀', '经典社交推理游戏，村民与狼人之间的对决，需要口才和逻辑推理', 8, 18, 45, '简单', ARRAY['推理','社交','身份隐藏'], '🐺', 512, 68, true),
    ('splendor', '璀璨宝石', '文艺复兴时期的宝石商人，收集宝石、发展产业、获得贵族青睐', 2, 4, 30, '简单', ARRAY['策略','卡牌','经济'], '💎', 128, 18, false),
    ('catan', '卡坦岛', '在卡坦岛上开拓殖民，通过交易和建设成为最成功的拓荒者', 3, 4, 60, '中等', ARRAY['策略','交易','建设'], '🏝️', 189, 24, true),
    ('codenames', '行动代号', '两队间谍通过代号猜测己方特工，考验联想和默契', 4, 8, 15, '简单', ARRAY['词汇','团队','推理'], '🕵️', 98, 12, false),
    ('unocards', 'UNO牌', '经典卡牌游戏，抢先出完手中所有牌，用功能牌逆转局势', 2, 10, 20, '简单', ARRAY['卡牌','休闲','派对'], '🃏', 367, 45, true)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(128) NOT NULL,
    game_id VARCHAR(64) REFERENCES games(id),
    host_id UUID REFERENCES users(id),
    max_players INT NOT NULL CHECK (max_players BETWEEN 2 AND 20),
    is_private BOOLEAN DEFAULT FALSE,
    password_hash TEXT,
    status VARCHAR(16) DEFAULT 'Waiting' CHECK (status IN ('Waiting', 'Playing', 'Finished')),
    game_mode VARCHAR(64) DEFAULT '经典',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS room_players (
    room_id UUID REFERENCES rooms(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id),
    is_ready BOOLEAN DEFAULT FALSE,
    is_host BOOLEAN DEFAULT FALSE,
    joined_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (room_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_room_players_room ON room_players(room_id);
CREATE INDEX IF NOT EXISTS idx_room_players_user ON room_players(user_id);

CREATE TABLE IF NOT EXISTS chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id UUID REFERENCES rooms(id) ON DELETE CASCADE,
    sender_id UUID REFERENCES users(id),
    content TEXT NOT NULL,
    is_system BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chat_room ON chat_messages(room_id, created_at DESC);
