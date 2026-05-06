use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RoomRow {
    pub id: Uuid,
    pub name: String,
    pub game_id: String,
    pub host_id: Uuid,
    pub max_players: i32,
    pub is_private: bool,
    pub password_hash: Option<String>,
    pub status: String,
    pub game_mode: String,
    pub created_at: DateTime<Utc>,
    pub short_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: Uuid,
    pub name: String,
    pub game_id: String,
    pub game_name: String,
    pub host_id: Uuid,
    pub host_name: String,
    pub players: Vec<RoomPlayer>,
    pub max_players: i32,
    pub is_private: bool,
    pub password_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    pub status: String,
    pub game_mode: String,
    pub created_at: DateTime<Utc>,
    pub has_password: bool,
    pub short_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RoomPlayer {
    pub user_id: Uuid,
    pub username: String,
    pub avatar: String,
    pub is_ready: bool,
    pub is_host: bool,
    pub is_online: bool,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoomPayload {
    pub name: String,
    pub game_id: String,
    pub game_name: String,
    pub max_players: i32,
    pub is_private: bool,
    pub password: Option<String>,
    pub game_mode: String,
}

#[derive(Debug, Deserialize)]
pub struct RoomFilter {
    pub game_type: Option<String>,
    pub keyword: Option<String>,
    pub status: Option<String>,
    pub game_mode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HomeStats {
    pub online_players: i32,
    pub active_rooms: i32,
    pub games_in_play: i32,
    pub hot_games: Vec<super::game::Game>,
    pub hot_rooms: Vec<Room>,
}
