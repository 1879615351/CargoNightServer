CREATE TABLE IF NOT EXISTS game_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id UUID NOT NULL,
    room_name VARCHAR(128) NOT NULL,
    game_type VARCHAR(64) DEFAULT 'avalon',
    winner VARCHAR(16) NOT NULL,
    assassin_target UUID,
    assassin_hit BOOLEAN,
    rounds_played INT DEFAULT 0,
    mission_results JSONB DEFAULT '[]',
    players JSONB DEFAULT '[]',
    round_history JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_game_records_created ON game_records(created_at DESC);

CREATE TABLE IF NOT EXISTS game_record_players (
    record_id UUID REFERENCES game_records(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id),
    role VARCHAR(64) NOT NULL,
    alignment VARCHAR(16) NOT NULL,
    won BOOLEAN NOT NULL,
    PRIMARY KEY (record_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_grp_user ON game_record_players(user_id, record_id DESC);
