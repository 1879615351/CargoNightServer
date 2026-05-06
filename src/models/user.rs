use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub avatar: String,
    pub bio: String,
    pub total_games: i32,
    pub win_rate: f32,
    pub favorite_game: String,
    pub created_at: DateTime<Utc>,
    pub account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserPublic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub avatar: String,
    pub bio: String,
    pub total_games: i32,
    pub win_rate: f32,
    pub favorite_game: String,
    pub created_at: DateTime<Utc>,
    pub account_id: Option<String>,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            avatar: u.avatar,
            bio: u.bio,
            total_games: u.total_games,
            win_rate: u.win_rate,
            favorite_game: u.favorite_game,
            created_at: u.created_at,
            account_id: u.account_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OnlineFriend {
    pub id: Uuid,
    pub username: String,
    pub avatar: String,
    pub status: String,
    pub current_game: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlatformAnnouncement {
    pub id: String,
    pub title: String,
    pub content: String,
    pub time: String,
}
