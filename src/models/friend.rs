use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FriendRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub friend_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub avatar: String,
    pub status: String,
    pub online_status: String,
    pub room_id: Option<Uuid>,
    pub room_name: Option<String>,
    pub game_name: Option<String>,
    pub game_mode: Option<String>,
    pub player_count: Option<i32>,
    pub max_players: Option<i32>,
    pub has_password: Option<bool>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AddFriendReq {
    pub friend_uid: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct AcceptFriendReq {
    pub request_id: Uuid,
}
