use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct InitRequest {
    pub public_key: String,
}

#[derive(Serialize)]
pub struct InitResponse {
    pub session_id: String,
    pub salt: String,
    pub opcodes_b64: String,
    pub initial_hash: String,
    pub expires_at: u64,
}

#[derive(Deserialize, Serialize)]
pub struct HeartbeatRequest {
    pub session_id: String,
    pub prev_hash: String,
    pub timestamp: u64,
    pub entropy_data: EntropyData,
    pub stack_state: StackState,
    pub fingerprint: Fingerprint,
    pub signature: String,
}

#[derive(Serialize)]
pub struct HeartbeatResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_salt: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Fingerprint {
    #[serde(rename = "aspectRatio")]
    pub aspect_ratio: String,
    #[serde(rename = "devicePixelRatio")]
    pub device_pixel_ratio: String,
    #[serde(rename = "hardwareConcurrency")]
    pub hardware_concurrency: u32,
}

#[derive(Deserialize, Serialize)]
pub struct EntropyData {
    pub events: Vec<MouseEvent>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct MouseEvent {
    pub x: f64,
    pub y: f64,
    #[serde(rename = "t")]
    pub timestamp_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackState {
    pub stack: Vec<u32>,
    pub ip: u16,
}