use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NtfyMessage {
    pub topic: String,
    pub title: Option<String>,
    pub message: String,
    pub priority: Option<u8>,
    pub tags: Option<Vec<String>>,
    pub click: Option<String>,
    pub attach: Option<String>,
    pub filename: Option<String>,
    pub delay: Option<String>,
    pub email: Option<String>,
    pub call: Option<String>,
    pub actions: Option<Vec<NtfyAction>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NtfyAction {
    pub action: String,
    pub label: String,
    pub url: Option<String>,
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    pub clear: Option<bool>,
}