use axum::{Router, routing::post, extract::{State, Path}, Json};
use uuid::Uuid;
use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
use argon2::Argon2;

use crate::db::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::error::AppError;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/rooms/{id}/ai/add", post(add_ai_players))
}

#[derive(serde::Deserialize)]
struct AddAIRequest {
    count: usize,
    difficulty: Option<String>,
}

async fn add_ai_players(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
    Json(body): Json<AddAIRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let count = body.count.min(9);
    if count == 0 { return Err(AppError::BadRequest("count required".into())); }

    let room = sqlx::query_as::<_, crate::models::room::RoomRow>("SELECT * FROM rooms WHERE id = $1")
        .bind(room_id).fetch_optional(&state.pool).await?
        .ok_or(AppError::NotFound("Room not found".into()))?;

    let current_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_players WHERE room_id = $1")
        .bind(room_id).fetch_one(&state.pool).await?;
    let max_add = (room.max_players as i64 - current_count).min(count as i64);
    if max_add <= 0 { return Err(AppError::BadRequest("Room is full".into())); }

    let difficulty = body.difficulty.unwrap_or_else(|| "normal".into());
    let ai_names = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Theta", "Sigma", "Omega"];

    let mut added = vec![];
    for i in 0..max_add as usize {
        let short_id = Uuid::new_v4().to_string()[..6].to_string();
        let ai_name = format!("AI-{}-{}", ai_names[i % ai_names.len()], short_id);
        let ai_email = format!("ai_{}@cargonight.ai", Uuid::new_v4());
        let pw = format!("ai_pass_{}", Uuid::new_v4());
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default().hash_password(pw.as_bytes(), &salt).map_err(|e| AppError::Internal(format!("hash: {}", e)))?.to_string();

        let user = sqlx::query_as::<_, crate::models::user::User>(
            "INSERT INTO users (username, email, password_hash, avatar) VALUES ($1, $2, $3, $4) RETURNING *"
        ).bind(&ai_name).bind(&ai_email).bind(&hash).bind("AI").fetch_one(&state.pool).await
        .map_err(|e| {
            tracing::error!("Failed to create AI user: {:?}", e);
            AppError::Internal(format!("Failed to create AI: {}", e))
        })?;

        sqlx::query("INSERT INTO room_players (room_id, user_id, is_ready, is_host) VALUES ($1, $2, true, false)")
            .bind(room_id).bind(user.id).execute(&state.pool).await
            .map_err(|e| {
                tracing::error!("Failed to add AI to room: {:?}", e);
                AppError::Internal(format!("Failed to add AI to room: {}", e))
            })?;

        added.push(serde_json::json!({"id": user.id, "name": ai_name, "avatar": "AI"}));
    }

    let row = sqlx::query_as::<_, crate::models::room::RoomRow>("SELECT * FROM rooms WHERE id = $1")
        .bind(room_id).fetch_one(&state.pool).await?;
    let room = crate::handlers::rooms::get_room_by_id(&state.pool, room_id).await?;
    state.ws_state.broadcast(room_id, crate::ws::manager::RoomEvent::PlayerJoined { room: room.clone() }).await;

    Ok(Json(serde_json::json!({"ok": true, "added": added, "count": added.len()})))
}
