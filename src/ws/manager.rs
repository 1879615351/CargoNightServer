use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use serde::Serialize;

use crate::models::room::Room;
use crate::models::chat::ChatMessage;
use crate::game::avalon::engine::PlayerGameView;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum RoomEvent {
    #[serde(rename = "chat")]
    NewMessage { message: ChatMessage },
    #[serde(rename = "player_joined")]
    PlayerJoined { room: Room },
    #[serde(rename = "player_left")]
    PlayerLeft { room: Room },
    #[serde(rename = "ready_changed")]
    ReadyChanged { room: Room },
    #[serde(rename = "game_started")]
    GameStarted { room: Room },
    #[serde(rename = "signaling")]
    Signaling {
        signal_type: String,
        sender_user_id: String,
        target_user_id: Option<String>,
        data: serde_json::Value,
    },
    #[serde(rename = "voice_state")]
    VoiceState {
        user_id: String,
        muted: bool,
    },
    #[serde(rename = "avalon_state")]
    AvalonState {
        views: HashMap<Uuid, PlayerGameView>,
    },
    #[serde(rename = "speak_start")]
    SpeakStart {
        player_id: Uuid,
        timeout: u64,
    },
    #[serde(rename = "speak_end")]
    SpeakEnd {
        player_id: Uuid,
    },
    #[serde(rename = "speak")]
    Speak {
        player_id: Uuid,
        content: String,
    },
    #[serde(rename = "stage_change")]
    StageChange {
        stage: String,
    },
}

#[derive(Clone)]
pub struct WsState {
    rooms: Arc<RwLock<HashMap<Uuid, broadcast::Sender<String>>>>,
}

impl WsState {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn subscribe(&self, room_id: Uuid) -> broadcast::Receiver<String> {
        let rooms = self.rooms.read().await;
        match rooms.get(&room_id) {
            Some(sender) => sender.subscribe(),
            None => {
                drop(rooms);
                let mut rooms = self.rooms.write().await;
                let (tx, rx) = broadcast::channel(256);
                rooms.insert(room_id, tx);
                rx
            }
        }
    }

    pub async fn broadcast(&self, room_id: Uuid, event: RoomEvent) {
        if let Ok(json) = serde_json::to_string(&event) {
            if let Some(tx) = self.rooms.read().await.get(&room_id) {
                let _ = tx.send(json);
            }
        }
    }

    pub async fn broadcast_avalon(&self, room_id: Uuid, views: HashMap<Uuid, PlayerGameView>) {
        let event = RoomEvent::AvalonState { views };
        if let Ok(json) = serde_json::to_string(&event) {
            if let Some(tx) = self.rooms.read().await.get(&room_id) {
                let _ = tx.send(json);
            }
        }
    }

    pub async fn remove_room(&self, room_id: Uuid) {
        self.rooms.write().await.remove(&room_id);
    }
}
