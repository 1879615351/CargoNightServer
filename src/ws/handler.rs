use axum::{
    Router, routing::get,
    extract::{ws::{WebSocket, WebSocketUpgrade, Message}, Query, State},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::AppState;
use crate::middleware::auth::Claims;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

#[derive(Deserialize)]
pub struct WsQuery {
    token: String,
    room_id: Option<Uuid>,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/ws", get(ws_handler))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
) -> impl IntoResponse {
    let token_data = match decode::<Claims>(
        &query.token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    ) {
        Ok(d) => d,
        Err(_) => return Err((axum::http::StatusCode::UNAUTHORIZED, "Invalid token").into_response()),
    };

    let user_id = match Uuid::parse_str(&token_data.claims.sub) {
        Ok(id) => id,
        Err(_) => return Err((axum::http::StatusCode::UNAUTHORIZED, "Invalid user").into_response()),
    };

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, user_id, query.room_id)))
}

async fn handle_socket(socket: WebSocket, state: AppState, _user_id: Uuid, room_id: Option<Uuid>) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));

    let sender_clone = sender.clone();
    let mut room_rx = if let Some(rid) = room_id {
        Some(state.ws_state.subscribe(rid).await)
    } else { None };

    let local_rx = if let Some(ref mut rx) = room_rx {
        let mut rx_clone = rx.resubscribe();
        let task = tokio::spawn(async move {
            while let Ok(msg) = rx_clone.recv().await {
                let mut s = sender_clone.lock().await;
                if s.send(Message::Text(msg.into())).await.is_err() { break; }
            }
        });
        Some(task)
    } else { None };

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Ping(data) => {
                let mut s = sender.lock().await;
                let _ = s.send(Message::Pong(data)).await;
            }
            Message::Text(_) => {},
            Message::Close(_) => break,
            _ => {}
        }
    }

    if let Some(task) = local_rx {
        task.abort();
    }
}
