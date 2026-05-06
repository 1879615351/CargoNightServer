use axum::{Router, routing::{get, post, delete}, extract::{State, Path, Query}, Json};
use uuid::Uuid;
use serde::Deserialize;

use crate::db::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::models::friend::{FriendRow, FriendInfo, AddFriendReq};

#[derive(Deserialize)]
struct SearchQuery { uid: Option<String> }

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/friends", get(get_friends))
        .route("/api/friends/add", post(add_friend))
        .route("/api/friends/accept", post(accept_friend))
        .route("/api/friends/{id}", delete(remove_friend))
        .route("/api/friends/search", get(search_user))
        .route("/api/users/{id}", get(get_user_info))
}

async fn get_friends(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
) -> Result<Json<Vec<FriendInfo>>, AppError> {
    // Return both accepted and pending requests
    let rows = sqlx::query_as::<_, FriendRow>(
        "SELECT * FROM friends WHERE (user_id = $1 OR friend_id = $1) AND status IN ('accepted', 'pending') ORDER BY created_at DESC"
    ).bind(auth.user_id).fetch_all(&state.pool).await?;

    let online = state.online_users.lock().await;
    let mut friends = Vec::new();

    for row in rows {
        let other_id = if row.user_id == auth.user_id { row.friend_id } else { row.user_id };
        let user_info = match sqlx::query_as::<_, (String, String)>("SELECT username, COALESCE(avatar, '🎮') FROM users WHERE id = $1")
            .bind(other_id).fetch_one(&state.pool).await {
            Ok(u) => u,
            Err(_) => ("?".into(), "🎮".into()),
        };
        
        let oinfo = online.get(&other_id);
        friends.push(FriendInfo {
            id: row.id, user_id: other_id,
            username: user_info.0, avatar: user_info.1,
            status: row.status.clone(),
            online_status: oinfo.map_or("offline".into(), |o| o.status.clone()),
            room_id: oinfo.and_then(|o| o.room_id),
            room_name: oinfo.and_then(|o| o.room_name.clone()),
            game_name: oinfo.and_then(|o| o.game_name.clone()),
            game_mode: oinfo.and_then(|o| o.game_mode.clone()),
            player_count: oinfo.and_then(|o| o.player_count),
            max_players: oinfo.and_then(|o| o.max_players),
            has_password: oinfo.and_then(|o| o.has_password),
            created_at: row.created_at.format("%Y-%m-%d").to_string(),
        });
    }
    Ok(Json(friends))
}

async fn add_friend(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(payload): Json<AddFriendReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    if payload.friend_uid == auth.user_id {
        return Err(AppError::BadRequest("Cannot add yourself".into()));
    }
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = $1")
        .bind(payload.friend_uid).fetch_one(&state.pool).await?;
    if exists == 0 { return Err(AppError::NotFound("User not found".into())); }

    let already: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM friends WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)"
    ).bind(auth.user_id).bind(payload.friend_uid).fetch_one(&state.pool).await?;
    if already > 0 { return Err(AppError::BadRequest("Already friends or request pending".into())); }

    sqlx::query("INSERT INTO friends (user_id, friend_id, status) VALUES ($1, $2, 'pending')")
        .bind(auth.user_id).bind(payload.friend_uid).execute(&state.pool).await?;

    Ok(Json(serde_json::json!({"ok": true})))
}

async fn accept_friend(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let request_id = Uuid::parse_str(body["request_id"].as_str().ok_or(AppError::BadRequest("request_id required".into()))?)
        .map_err(|_| AppError::BadRequest("invalid id".into()))?;

    sqlx::query("UPDATE friends SET status = 'accepted' WHERE id = $1 AND friend_id = $2")
        .bind(request_id).bind(_auth.user_id).execute(&state.pool).await?;

    Ok(Json(serde_json::json!({"ok": true})))
}

async fn remove_friend(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(friend_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query("DELETE FROM friends WHERE id = $1 AND (user_id = $2 OR friend_id = $2)")
        .bind(friend_id).bind(auth.user_id).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn search_user(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Query(q): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let input = q.uid.unwrap_or_default();
    if input.is_empty() { return Err(AppError::BadRequest("input required".into())); }

    // Search by account_id first (12-digit number)
    let user = sqlx::query_as::<_, (String, String, String)>("SELECT id::text, username, COALESCE(avatar, '🎮') FROM users WHERE account_id = $1")
        .bind(&input).fetch_optional(&state.pool).await.ok().flatten();

    // Fallback: search by username or UUID
    let user = user.or_else(|| {
        // Try username first (sync fallback)
        None
    });

    let user = if user.is_none() {
        sqlx::query_as::<_, (String, String, String)>("SELECT id::text, username, COALESCE(avatar, '🎮') FROM users WHERE username ILIKE $1 LIMIT 1")
            .bind(format!("%{}%", &input)).fetch_optional(&state.pool).await.ok().flatten()
    } else { user };

    let user = if user.is_none() {
        if let Ok(uid) = Uuid::parse_str(&input) {
            sqlx::query_as::<_, (String, String, String)>("SELECT id::text, username, COALESCE(avatar, '🎮') FROM users WHERE id = $1")
                .bind(uid).fetch_optional(&state.pool).await.ok().flatten()
        } else { None }
    } else { user };

    match user {
        Some((id, name, avatar)) => Ok(Json(serde_json::json!({"found": true, "user": {"id": id, "username": name, "avatar": avatar}}))),
        None => Ok(Json(serde_json::json!({"found": false}))),
    }
}

async fn get_user_info(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user = sqlx::query_as::<_, (String, String)>("SELECT username, COALESCE(avatar, '🎮') FROM users WHERE id = $1")
        .bind(user_id).fetch_one(&state.pool).await
        .map_err(|_| AppError::NotFound("User not found".into()))?;

    let online = state.online_users.lock().await;
    let info = online.get(&user_id);

    Ok(Json(serde_json::json!({
        "id": user_id, "username": user.0, "avatar": user.1,
        "online_status": info.map_or("offline".to_string(), |o| o.status.clone()),
        "room_id": info.and_then(|o| o.room_id),
        "room_name": info.and_then(|o| o.room_name.clone()),
        "game_name": info.and_then(|o| o.game_name.clone()),
    })))
}
