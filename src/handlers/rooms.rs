use axum::{Router, routing::{get, post}, extract::{State, Path, Query}, Json};
use uuid::Uuid;
use rand::Rng;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};

use crate::models::room::{Room, RoomPlayer, CreateRoomPayload, RoomFilter, RoomRow};
use crate::middleware::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::db::AppState;
use crate::ws::manager::RoomEvent;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/lobby/rooms", get(get_lobby_rooms))
        .route("/api/rooms", post(create_room))
        .route("/api/rooms/{id}/join", post(join_room))
        .route("/api/rooms/{id}/leave", post(leave_room))
        .route("/api/rooms/{id}/ready", post(toggle_ready))
        .route("/api/rooms/{id}/start", post(start_game))
}

async fn enrich_room(pool: &sqlx::PgPool, row: RoomRow) -> Result<Room, sqlx::Error> {
    let host_name: String = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(row.host_id).fetch_one(pool).await?;
    let game_name: String = sqlx::query_scalar("SELECT name FROM games WHERE id = $1")
        .bind(&row.game_id).fetch_one(pool).await?;
    let players = sqlx::query_as::<_, RoomPlayer>(
        r#"SELECT u.id as user_id, u.username, u.avatar, rp.is_ready, rp.is_host,
                  true as is_online, rp.joined_at
           FROM room_players rp JOIN users u ON rp.user_id = u.id
           WHERE rp.room_id = $1"#
    ).bind(row.id).fetch_all(pool).await?;

    let has_password = row.password_hash.is_some();
    Ok(Room {
        id: row.id, name: row.name, game_id: row.game_id, game_name,
        host_id: row.host_id, host_name, players,
        max_players: row.max_players, is_private: row.is_private,
        password_hash: row.password_hash, password: None,
        status: row.status, game_mode: row.game_mode,
        created_at: row.created_at, has_password, short_id: row.short_id,
    })
}

async fn get_lobby_rooms(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Query(filter): Query<RoomFilter>,
) -> Result<Json<Vec<Room>>, AppError> {
    let mut query = String::from("SELECT * FROM rooms WHERE status != 'Finished'");
    let mut params: Vec<String> = vec![];

    if let Some(ref gt) = filter.game_type {
        if !gt.is_empty() {
            query.push_str(&format!(" AND game_id = ${}", params.len() + 1));
            params.push(gt.clone());
        }
    }
    if let Some(ref st) = filter.status {
        if st == "waiting" || st == "playing" {
            let s = if st == "waiting" { "Waiting" } else { "Playing" };
            query.push_str(&format!(" AND status = ${}", params.len() + 1));
            params.push(s.into());
        }
    }
    if let Some(ref gm) = filter.game_mode {
        if !gm.is_empty() {
            query.push_str(&format!(" AND game_mode = ${}", params.len() + 1));
            params.push(gm.clone());
        }
    }
    query.push_str(" ORDER BY created_at DESC");

    let mut db_query = sqlx::query_as::<_, RoomRow>(&query);
    for p in &params {
        db_query = db_query.bind(p);
    }
    let rows = db_query.fetch_all(&state.pool).await?;

    let mut rooms = Vec::new();
    for r in rows {
        if let Some(ref kw) = filter.keyword {
            if !kw.is_empty() {
                let host_name: String = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
                    .bind(r.host_id).fetch_one(&state.pool).await.unwrap_or_default();
                if !r.name.to_lowercase().contains(&kw.to_lowercase())
                    && !host_name.to_lowercase().contains(&kw.to_lowercase()) {
                    continue;
                }
            }
        }
        if let Ok(enriched) = enrich_room(&state.pool, r).await {
            rooms.push(enriched);
        }
    }

    Ok(Json(rooms))
}

async fn create_room(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(payload): Json<CreateRoomPayload>,
) -> Result<Json<Room>, AppError> {
    tracing::info!("create_room: name={} game_id={} user={}", payload.name, payload.game_id, auth.user_id);
    let password_hash = if payload.is_private && !payload.password.as_ref().map_or(true, |p| p.is_empty()) {
        let salt = SaltString::generate(&mut OsRng);
        Some(Argon2::default()
            .hash_password(payload.password.as_ref().unwrap().as_bytes(), &salt)
            .map_err(|e| AppError::Internal(format!("Hash: {}", e)))?
            .to_string())
    } else { None };

    // Generate unique 6-digit short_id
    let short_id = loop {
        let candidate: String = (0..6).map(|_| rand::thread_rng().gen_range(0..10).to_string()).collect();
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rooms WHERE short_id = $1")
            .bind(&candidate).fetch_one(&state.pool).await.unwrap_or(0);
        if exists == 0 { break candidate; }
    };

    let row = sqlx::query_as::<_, RoomRow>(
        "INSERT INTO rooms (name, game_id, host_id, max_players, is_private, password_hash, game_mode, short_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *"
    ).bind(&payload.name).bind(&payload.game_id).bind(auth.user_id)
     .bind(payload.max_players).bind(payload.is_private).bind(&password_hash).bind(&payload.game_mode)
     .bind(&short_id)
     .fetch_one(&state.pool).await
     .map_err(|e| {
         tracing::error!("Failed to insert room: {:?}", e);
         AppError::Internal(format!("Failed to create room: {}", e))
     })?;

    sqlx::query("INSERT INTO room_players (room_id, user_id, is_ready, is_host) VALUES ($1, $2, true, true)")
        .bind(row.id).bind(auth.user_id).execute(&state.pool).await
        .map_err(|e| {
            tracing::error!("Failed to insert room_player: {:?}", e);
            AppError::Internal(format!("Failed to add player: {}", e))
        })?;

    enrich_room(&state.pool, row).await.map(Json).map_err(|e| {
        tracing::error!("Failed to enrich room: {:?}", e);
        AppError::Internal(format!("Failed to enrich: {}", e))
    })
}

pub async fn get_room_by_id(pool: &sqlx::PgPool, room_id: uuid::Uuid) -> Result<Room, AppError> {
    let row = sqlx::query_as::<_, RoomRow>("SELECT * FROM rooms WHERE id = $1")
        .bind(room_id).fetch_optional(pool).await?
        .ok_or(AppError::NotFound("Room not found".into()))?;
    enrich_room(pool, row).await.map_err(AppError::from)
}

async fn join_room(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Room>, AppError> {
    let row = sqlx::query_as::<_, RoomRow>("SELECT * FROM rooms WHERE id = $1")
        .bind(room_id).fetch_optional(&state.pool).await?
        .ok_or(AppError::NotFound("Room not found".into()))?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_players WHERE room_id = $1")
        .bind(room_id).fetch_one(&state.pool).await?;
    if count >= row.max_players as i64 {
        return Err(AppError::BadRequest("Room is full".into()));
    }

    let already: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_players WHERE room_id = $1 AND user_id = $2")
        .bind(room_id).bind(auth.user_id).fetch_one(&state.pool).await?;
    if already > 0 {
        return Err(AppError::Conflict("Already in room".into()));
    }

    sqlx::query("INSERT INTO room_players (room_id, user_id, is_ready, is_host) VALUES ($1, $2, false, false)")
        .bind(room_id).bind(auth.user_id).execute(&state.pool).await?;

    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(auth.user_id).fetch_one(&state.pool).await?;
    sqlx::query("INSERT INTO chat_messages (room_id, sender_id, content, is_system) VALUES ($1, $2, $3, true)")
        .bind(room_id).bind(auth.user_id).bind(format!("{} 加入了房间", username))
        .execute(&state.pool).await?;

    let room = enrich_room(&state.pool, row).await?;
    state.ws_state.broadcast(room_id, RoomEvent::PlayerJoined { room: room.clone() }).await;
    Ok(Json(room))
}

async fn leave_room(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Block leave during active Avalon game
    let game_active: bool = {
        let games = state.avalon_games.lock().await;
        games.get(&room_id).map_or(false, |g| g.phase != crate::game::avalon::engine::GamePhase::End
            && g.phase != crate::game::avalon::engine::GamePhase::End)
    };
    if game_active {
        return Err(AppError::BadRequest("Cannot leave during active game".into()));
    }

    let was_host: bool = sqlx::query_scalar(
        "SELECT is_host FROM room_players WHERE room_id = $1 AND user_id = $2"
    ).bind(room_id).bind(auth.user_id).fetch_optional(&state.pool).await?.unwrap_or(false);

    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(auth.user_id).fetch_one(&state.pool).await?;

    sqlx::query("DELETE FROM room_players WHERE room_id = $1 AND user_id = $2")
        .bind(room_id).bind(auth.user_id).execute(&state.pool).await?;
    sqlx::query("INSERT INTO chat_messages (room_id, sender_id, content, is_system) VALUES ($1, $2, $3, true)")
        .bind(room_id).bind(auth.user_id).bind(format!("{} 离开了房间", username))
        .execute(&state.pool).await?;

    if was_host {
        let next_host: Option<(Uuid,)> = sqlx::query_as(
            "SELECT user_id FROM room_players WHERE room_id = $1 ORDER BY joined_at LIMIT 1"
        ).bind(room_id).fetch_optional(&state.pool).await?;

        if let Some((next_id,)) = next_host {
            sqlx::query("UPDATE room_players SET is_host = true, is_ready = true WHERE room_id = $1 AND user_id = $2")
                .bind(room_id).bind(next_id).execute(&state.pool).await?;
            sqlx::query("UPDATE rooms SET host_id = $2 WHERE id = $1")
                .bind(room_id).bind(next_id).execute(&state.pool).await?;
        } else {
            sqlx::query("DELETE FROM rooms WHERE id = $1").bind(room_id).execute(&state.pool).await?;
            return Ok(Json(serde_json::json!({"ok": true, "closed": true})));
        }
    }

    // Close room if no human players remain (AI-only doesn't count)
    let human_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM room_players rp JOIN users u ON rp.user_id = u.id WHERE rp.room_id = $1 AND u.avatar != 'AI'"
    ).bind(room_id).fetch_one(&state.pool).await?;
    if human_count == 0 {
        sqlx::query("DELETE FROM rooms WHERE id = $1").bind(room_id).execute(&state.pool).await?;
        return Ok(Json(serde_json::json!({"ok": true, "closed": true})));
    }

    if let Ok(r) = sqlx::query_as::<_, RoomRow>("SELECT * FROM rooms WHERE id = $1").bind(room_id).fetch_optional(&state.pool).await {
        if let Some(r) = r {
            let room = enrich_room(&state.pool, r).await?;
            state.ws_state.broadcast(room_id, RoomEvent::PlayerLeft { room: room.clone() }).await;
        }
    }

    Ok(Json(serde_json::json!({"ok": true})))
}

async fn toggle_ready(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Room>, AppError> {
    let current_ready: bool = sqlx::query_scalar(
        "SELECT is_ready FROM room_players WHERE room_id = $1 AND user_id = $2"
    ).bind(room_id).bind(auth.user_id).fetch_optional(&state.pool).await?
        .ok_or(AppError::NotFound("Not in room".into()))?;

    sqlx::query("UPDATE room_players SET is_ready = NOT is_ready WHERE room_id = $1 AND user_id = $2")
        .bind(room_id).bind(auth.user_id).execute(&state.pool).await?;

    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(auth.user_id).fetch_one(&state.pool).await?;
    sqlx::query("INSERT INTO chat_messages (room_id, sender_id, content, is_system) VALUES ($1, $2, $3, true)")
        .bind(room_id).bind(auth.user_id)
        .bind(format!("{} {}", username, if !current_ready { "已准备" } else { "取消准备" }))
        .execute(&state.pool).await?;

    let row = sqlx::query_as::<_, RoomRow>("SELECT * FROM rooms WHERE id = $1")
        .bind(room_id).fetch_one(&state.pool).await?;
    let room = enrich_room(&state.pool, row).await?;
    state.ws_state.broadcast(room_id, RoomEvent::ReadyChanged { room: room.clone() }).await;
    Ok(Json(room))
}

async fn start_game(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Room>, AppError> {
    let is_host: bool = sqlx::query_scalar(
        "SELECT is_host FROM room_players WHERE room_id = $1 AND user_id = $2"
    ).bind(room_id).bind(auth.user_id).fetch_optional(&state.pool).await?.unwrap_or(false);
    if !is_host { return Err(AppError::BadRequest("Only host can start".into())); }

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_players WHERE room_id = $1")
        .bind(room_id).fetch_one(&state.pool).await?;
    let ready: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_players WHERE room_id = $1 AND is_ready = true")
        .bind(room_id).fetch_one(&state.pool).await?;
    if ready < total { return Err(AppError::BadRequest(format!("Not all ready ({}/{})", ready, total))); }
    if total < 2 { return Err(AppError::BadRequest("Need at least 2 players".into())); }

    sqlx::query("UPDATE rooms SET status = 'Playing' WHERE id = $1")
        .bind(room_id).execute(&state.pool).await?;
    sqlx::query("INSERT INTO chat_messages (room_id, sender_id, content, is_system) VALUES ($1, $2, '游戏开始！', true)")
        .bind(room_id).bind(auth.user_id).execute(&state.pool).await?;

    let row = sqlx::query_as::<_, RoomRow>("SELECT * FROM rooms WHERE id = $1")
        .bind(room_id).fetch_one(&state.pool).await?;
    let room = enrich_room(&state.pool, row).await?;
    state.ws_state.broadcast(room_id, RoomEvent::GameStarted { room: room.clone() }).await;
    Ok(Json(room))
}
