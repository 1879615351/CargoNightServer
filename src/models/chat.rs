use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatMessage {
    pub id: Uuid,
    pub room_id: Uuid,
    pub sender_id: Uuid,
    pub sender_name: String,
    pub content: String,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatPayload {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatQuery {
    pub before: Option<i64>,
    pub limit: Option<i64>,
}
