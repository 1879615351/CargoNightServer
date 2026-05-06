use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SignalingMessage {
    #[serde(rename = "type")]
    pub signal_type: String,
    pub room_id: String,
    pub target_user_id: Option<String>,
    pub sdp: Option<String>,
    pub candidate: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SignalingOut {
    #[serde(rename = "type")]
    pub signal_type: String,
    pub sender_user_id: String,
    pub target_user_id: Option<String>,
    pub sdp: Option<String>,
    pub candidate: Option<String>,
}
