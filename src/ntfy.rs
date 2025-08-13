use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[allow(dead_code)]
pub struct NtfyClient {
    client: Client,
    base_url: String,
    auth_token: Option<String>,
    send_format: String, // "text" or "json"
}

#[allow(dead_code)]
impl NtfyClient {
    pub fn new(
        base_url: String,
        auth_token: Option<String>,
        timeout_secs: Option<u64>,
        send_format: String,
    ) -> Result<Self> {
        let timeout = Duration::from_secs(timeout_secs.unwrap_or(30));

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(NtfyClient {
            client,
            base_url,
            auth_token,
            send_format,
        })
    }

    pub fn send(&self, message: &NtfyMessage) -> Result<()> {
        let url = self.build_url(&message.topic)?;
        let mut headers = HeaderMap::new();

        // Add authorization if configured
        if let Some(token) = &self.auth_token {
            let auth_value = format!("Bearer {token}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth_value).context("Invalid auth token")?,
            );
        }

        // Send based on format preference
        let response = if self.send_format == "json" {
            // JSON mode - send structured data
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let body = self.build_message_body(message)?;

            self.client
                .post(url)
                .headers(headers)
                .json(&body)
                .send()
                .context("Failed to send notification")?
        } else {
            // Text mode - send plain text with headers
            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            );

            // Add title as header if present
            if let Some(title) = &message.title {
                headers.insert(
                    "X-Title",
                    HeaderValue::from_str(title).context("Invalid title value")?,
                );
            }

            // Add priority as header if present
            if let Some(priority) = message.priority {
                headers.insert(
                    "X-Priority",
                    HeaderValue::from_str(&priority.to_string())
                        .context("Invalid priority value")?,
                );
            }

            // Add tags as header if present
            if let Some(tags) = &message.tags {
                let tags_str = tags.join(",");
                headers.insert(
                    "X-Tags",
                    HeaderValue::from_str(&tags_str).context("Invalid tags value")?,
                );
            }

            // Add other optional headers
            if let Some(click) = &message.click {
                headers.insert(
                    "X-Click",
                    HeaderValue::from_str(click).context("Invalid click value")?,
                );
            }

            if let Some(attach) = &message.attach {
                headers.insert(
                    "X-Attach",
                    HeaderValue::from_str(attach).context("Invalid attach value")?,
                );
            }

            if let Some(delay) = &message.delay {
                headers.insert(
                    "X-Delay",
                    HeaderValue::from_str(delay).context("Invalid delay value")?,
                );
            }

            if let Some(email) = &message.email {
                headers.insert(
                    "X-Email",
                    HeaderValue::from_str(email).context("Invalid email value")?,
                );
            }

            if let Some(call) = &message.call {
                headers.insert(
                    "X-Call",
                    HeaderValue::from_str(call).context("Invalid call value")?,
                );
            }

            // Send message text directly in body
            self.client
                .post(url)
                .headers(headers)
                .body(message.message.clone())
                .send()
                .context("Failed to send notification")?
        };

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to send notification: {} - {}", status, error_text);
        }

        Ok(())
    }

    pub fn send_simple(&self, topic: &str, title: &str, message: &str, priority: u8) -> Result<()> {
        let msg = NtfyMessage {
            topic: topic.to_string(),
            title: Some(title.to_string()),
            message: message.to_string(),
            priority: Some(priority),
            tags: None,
            click: None,
            attach: None,
            filename: None,
            delay: None,
            email: None,
            call: None,
            actions: None,
        };

        self.send(&msg)
    }

    fn build_url(&self, topic: &str) -> Result<String> {
        let base = Url::parse(&self.base_url).context("Invalid base URL")?;

        let url = base.join(topic).context("Failed to build topic URL")?;

        Ok(url.to_string())
    }

    fn build_message_body(&self, message: &NtfyMessage) -> Result<serde_json::Value> {
        let mut body = serde_json::json!({
            "message": message.message,
        });

        if let Some(title) = &message.title {
            body["title"] = serde_json::json!(title);
        }

        if let Some(priority) = message.priority {
            body["priority"] = serde_json::json!(priority);
        }

        if let Some(tags) = &message.tags {
            body["tags"] = serde_json::json!(tags);
        }

        if let Some(click) = &message.click {
            body["click"] = serde_json::json!(click);
        }

        if let Some(attach) = &message.attach {
            body["attach"] = serde_json::json!(attach);
        }

        if let Some(filename) = &message.filename {
            body["filename"] = serde_json::json!(filename);
        }

        if let Some(delay) = &message.delay {
            body["delay"] = serde_json::json!(delay);
        }

        if let Some(email) = &message.email {
            body["email"] = serde_json::json!(email);
        }

        if let Some(call) = &message.call {
            body["call"] = serde_json::json!(call);
        }

        if let Some(actions) = &message.actions {
            body["actions"] = serde_json::json!(actions);
        }

        Ok(body)
    }
}

// Async version for the daemon
#[allow(dead_code)]
pub struct AsyncNtfyClient {
    client: reqwest::Client,
    base_url: String,
    auth_token: Option<String>,
    send_format: String, // "text" or "json"
}

#[allow(dead_code)]
impl AsyncNtfyClient {
    pub fn new(
        base_url: String,
        auth_token: Option<String>,
        timeout_secs: Option<u64>,
        send_format: String,
    ) -> Result<Self> {
        let timeout = Duration::from_secs(timeout_secs.unwrap_or(30));

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .context("Failed to create async HTTP client")?;

        Ok(AsyncNtfyClient {
            client,
            base_url,
            auth_token,
            send_format,
        })
    }

    pub async fn send(&self, message: &NtfyMessage) -> Result<()> {
        let url = self.build_url(&message.topic)?;
        let mut headers = HeaderMap::new();

        // Add authorization if configured
        if let Some(token) = &self.auth_token {
            let auth_value = format!("Bearer {token}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth_value).context("Invalid auth token")?,
            );
        }

        // Send based on format preference
        let response = if self.send_format == "json" {
            // JSON mode - send structured data
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let body = self.build_message_body(message)?;

            self.client
                .post(url)
                .headers(headers)
                .json(&body)
                .send()
                .await
                .context("Failed to send async notification")?
        } else {
            // Text mode - send plain text with headers
            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            );

            // Add title as header if present
            if let Some(title) = &message.title {
                headers.insert(
                    "X-Title",
                    HeaderValue::from_str(title).context("Invalid title value")?,
                );
            }

            // Add priority as header if present
            if let Some(priority) = message.priority {
                headers.insert(
                    "X-Priority",
                    HeaderValue::from_str(&priority.to_string())
                        .context("Invalid priority value")?,
                );
            }

            // Add tags as header if present
            if let Some(tags) = &message.tags {
                let tags_str = tags.join(",");
                headers.insert(
                    "X-Tags",
                    HeaderValue::from_str(&tags_str).context("Invalid tags value")?,
                );
            }

            // Add other optional headers
            if let Some(click) = &message.click {
                headers.insert(
                    "X-Click",
                    HeaderValue::from_str(click).context("Invalid click value")?,
                );
            }

            if let Some(attach) = &message.attach {
                headers.insert(
                    "X-Attach",
                    HeaderValue::from_str(attach).context("Invalid attach value")?,
                );
            }

            if let Some(delay) = &message.delay {
                headers.insert(
                    "X-Delay",
                    HeaderValue::from_str(delay).context("Invalid delay value")?,
                );
            }

            if let Some(email) = &message.email {
                headers.insert(
                    "X-Email",
                    HeaderValue::from_str(email).context("Invalid email value")?,
                );
            }

            if let Some(call) = &message.call {
                headers.insert(
                    "X-Call",
                    HeaderValue::from_str(call).context("Invalid call value")?,
                );
            }

            // Send message text directly in body
            self.client
                .post(url)
                .headers(headers)
                .body(message.message.clone())
                .send()
                .await
                .context("Failed to send async notification")?
        };

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to send notification: {} - {}", status, error_text);
        }

        Ok(())
    }

    fn build_url(&self, topic: &str) -> Result<String> {
        let base = Url::parse(&self.base_url).context("Invalid base URL")?;

        let url = base.join(topic).context("Failed to build topic URL")?;

        Ok(url.to_string())
    }

    fn build_message_body(&self, message: &NtfyMessage) -> Result<serde_json::Value> {
        let mut body = serde_json::json!({
            "message": message.message,
        });

        if let Some(title) = &message.title {
            body["title"] = serde_json::json!(title);
        }

        if let Some(priority) = message.priority {
            body["priority"] = serde_json::json!(priority);
        }

        if let Some(tags) = &message.tags {
            body["tags"] = serde_json::json!(tags);
        }

        if let Some(click) = &message.click {
            body["click"] = serde_json::json!(click);
        }

        if let Some(attach) = &message.attach {
            body["attach"] = serde_json::json!(attach);
        }

        if let Some(filename) = &message.filename {
            body["filename"] = serde_json::json!(filename);
        }

        if let Some(delay) = &message.delay {
            body["delay"] = serde_json::json!(delay);
        }

        if let Some(email) = &message.email {
            body["email"] = serde_json::json!(email);
        }

        if let Some(call) = &message.call {
            body["call"] = serde_json::json!(call);
        }

        if let Some(actions) = &message.actions {
            body["actions"] = serde_json::json!(actions);
        }

        Ok(body)
    }
}
