use axum::{Router, routing::{get, post}, extract::{State, Path, Query}, Json};
use uuid::Uuid;

use crate::models::chat::{ChatMessage, ChatPayload, ChatQuery};
use crate::middleware::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::db::AppState;
use crate::ws::manager::RoomEvent;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/rooms/{id}/chat", get(get_chat).post(send_chat))
}

async fn get_chat(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
    Query(q): Query<ChatQuery>,
) -> Result<Json<Vec<ChatMessage>>, AppError> {
    let limit = q.limit.unwrap_or(50).min(100);
    let messages = sqlx::query_as::<_, ChatMessage>(
        r#"SELECT cm.id, cm.room_id, cm.sender_id, u.username as sender_name,
                  cm.content, cm.is_system, cm.created_at,
                  to_char(cm.created_at, 'HH24:MI:SS') as timestamp
           FROM chat_messages cm JOIN users u ON cm.sender_id = u.id
           WHERE cm.room_id = $1 ORDER BY cm.created_at ASC LIMIT $2"#
    ).bind(room_id).bind(limit).fetch_all(&state.pool).await?;
    Ok(Json(messages))
}

async fn send_chat(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<ChatPayload>,
) -> Result<Json<ChatMessage>, AppError> {
    let msg = sqlx::query_as::<_, ChatMessage>(
        r#"INSERT INTO chat_messages (room_id, sender_id, content, is_system)
           VALUES ($1, $2, $3, false)
           RETURNING id, room_id, sender_id, '' as sender_name, content, is_system, created_at, NULL as timestamp"#
    ).bind(room_id).bind(auth.user_id).bind(&payload.content).fetch_one(&state.pool).await?;

    let enriched = sqlx::query_as::<_, ChatMessage>(
        r#"SELECT cm.id, cm.room_id, cm.sender_id, u.username as sender_name,
                  cm.content, cm.is_system, cm.created_at,
                  to_char(cm.created_at, 'HH24:MI:SS') as timestamp
           FROM chat_messages cm JOIN users u ON cm.sender_id = u.id
           WHERE cm.id = $1"#
    ).bind(msg.id).fetch_one(&state.pool).await?;

    state.ws_state.broadcast(room_id, RoomEvent::NewMessage { message: enriched.clone() }).await;
    Ok(Json(enriched))
}
