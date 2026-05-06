use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GameRecordRow {
    pub id: Uuid,
    pub room_name: String,
    pub game_type: String,
    pub winner: String,
    pub assassin_target: Option<Uuid>,
    pub assassin_hit: Option<bool>,
    pub rounds_played: i32,
    pub mission_results: serde_json::Value,
    pub players: serde_json::Value,
    pub round_history: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRecord {
    pub id: Uuid,
    pub room_name: String,
    pub game_type: String,
    pub winner: String,
    pub assassin_target: Option<Uuid>,
    pub assassin_hit: Option<bool>,
    pub rounds_played: i32,
    pub mission_results: serde_json::Value,
    pub players: serde_json::Value,
    pub round_history: serde_json::Value,
    pub created_at: String,
    pub your_role: Option<String>,
    pub you_won: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStats {
    pub total_games: i64,
    pub wins: i64,
    pub win_rate: f64,
    pub favorite_role: Option<String>,
    pub recent_records: Vec<GameRecord>,
}
