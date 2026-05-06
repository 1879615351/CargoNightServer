use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub description: String,
    pub min_players: i32,
    pub max_players: i32,
    pub duration_minutes: i32,
    pub difficulty: String,
    pub tags: Vec<String>,
    pub icon: String,
    pub online_count: i32,
    pub room_count: i32,
    pub hot: bool,
}
