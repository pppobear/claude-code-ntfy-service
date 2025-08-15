use anyhow::{Context, Result};
use flume::Receiver;
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

// Import specific items from daemon modules
use super::templates::{MessageFormatter, TemplateEngine};
use crate::shared::clients::{traits::NotificationClient, AsyncNtfyClient};
use crate::ntfy::NtfyMessage;
use super::shared::NotificationTask;


// NotificationTask is now imported from shared module

pub struct NotificationDaemon {
    template_engine: Arc<TemplateEngine>,
    message_formatter: Arc<MessageFormatter>,
    task_receiver: Receiver<NotificationTask>,
    shutdown_receiver: Receiver<()>,
    queue_size: Arc<AtomicUsize>,
    max_retries: u32,
    retry_delay: Duration,
}

impl NotificationDaemon {
    pub fn new(
        task_receiver: Receiver<NotificationTask>,
        shutdown_receiver: Receiver<()>,
        queue_size: Arc<AtomicUsize>,
    ) -> Result<Self> {
        let template_engine = Arc::new(TemplateEngine::new()?);
        let message_formatter = Arc::new(MessageFormatter::default());

        Ok(NotificationDaemon {
            template_engine,
            message_formatter,
            task_receiver,
            shutdown_receiver,
            queue_size,
            max_retries: 3, // Default retry attempts
            retry_delay: Duration::from_secs(5), // Default retry delay
        })
    }

    pub async fn run(self) -> Result<()> {
        info!("Notification daemon started");

        loop {
            tokio::select! {
                // Handle incoming notification tasks
                task = self.receive_task() => {
                    if let Some(task) = task {
                        self.process_task(task).await;
                    }
                }

                // Handle IPC shutdown signal
                _ = self.shutdown_receiver.recv_async() => {
                    info!("Received shutdown signal, stopping notification daemon");
                    break;
                }
            }
        }

        // Process remaining tasks before shutdown
        self.drain_queue().await;

        info!("Notification daemon stopped");
        Ok(())
    }

    async fn receive_task(&self) -> Option<NotificationTask> {
        match self.task_receiver.recv_async().await.ok() {
            Some(task) => {
                // Decrement queue size when task is dequeued
                self.queue_size.fetch_sub(1, Ordering::Relaxed);
                Some(task)
            }
            None => None,
        }
    }

    async fn process_task(&self, task: NotificationTask) {
        debug!("Processing notification task: {} from project: {:?}", 
               task.hook_name, task.project_path);

        // Deserialize hook data from JSON string
        let hook_data: serde_json::Value = match serde_json::from_str(&task.hook_data) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to deserialize hook data: {}", e);
                return;
            }
        };

        // Create dynamic ntfy client based on task configuration
        let ntfy_client = match self.create_ntfy_client(&task.ntfy_config).await {
            Ok(client) => client,
            Err(e) => {
                error!("Failed to create ntfy client for task {}: {}", task.hook_name, e);
                return;
            }
        };

        // Prepare notification message
        let message = match self.prepare_message(&task, &hook_data).await {
            Ok(msg) => msg,
            Err(e) => {
                error!(
                    "Failed to prepare message for hook {}: {}",
                    task.hook_name, e
                );
                return;
            }
        };

        // Send notification with retry logic
        let mut attempt = 0;
        loop {
            match ntfy_client.send(&message).await {
                Ok(_) => {
                    info!(
                        "Successfully sent notification for hook: {}",
                        task.hook_name
                    );
                    break;
                }
                Err(e) => {
                    attempt += 1;
                    if attempt > self.max_retries {
                        error!(
                            "Failed to send notification for hook {} after {} attempts: {}",
                            task.hook_name, self.max_retries, e
                        );
                        break;
                    }

                    warn!(
                        "Failed to send notification for hook {} (attempt {}/{}): {}",
                        task.hook_name, attempt, self.max_retries, e
                    );

                    sleep(self.retry_delay).await;
                }
            }
        }
    }

    /// Create ntfy client dynamically based on task configuration
    async fn create_ntfy_client(&self, config: &super::shared::NtfyTaskConfig) -> Result<AsyncNtfyClient> {
        use crate::shared::clients::ntfy::NtfyClientConfig;
        use crate::shared::clients::traits::RetryConfig;
        
        let client_config = NtfyClientConfig {
            server_url: config.server_url.clone(),
            auth_token: config.auth_token.clone(),
            timeout_secs: Some(30), // Default timeout
            send_format: config.send_format.clone(),
            retry_config: RetryConfig::exponential(3, 1000), // 3 retries, 1s base delay
            user_agent: Some("claude-ntfy-service".to_string()),
        };

        let client = AsyncNtfyClient::new(client_config)
            .context("Failed to create ntfy client")?;
        
        Ok(client)
    }

    async fn prepare_message(&self, task: &NotificationTask, hook_data: &serde_json::Value) -> Result<NtfyMessage> {

        // Get template name and render message body  
        let template_name = task.hook_name.replace('_', "-");
        let formatted_data = self
            .template_engine
            .format_hook_data(&task.hook_name, &hook_data);

        // Use default template rendering (no custom templates in global daemon)
        let body = self.template_engine
            .render(&template_name, &formatted_data, None)
            .unwrap_or_else(|_| {
                // Fallback to simple message if template fails
                format!("Hook: {}\nData: {}", task.hook_name, hook_data)
            });

        // Format title
        let title = self
            .message_formatter
            .format_title(&task.hook_name, &formatted_data);

        // Get configuration from task (no longer from config_manager)
        let topic = &task.ntfy_config.topic;
        let priority = task.ntfy_config.priority.unwrap_or(3);
        let tags = task.ntfy_config.tags.clone();

        Ok(NtfyMessage {
            topic: topic.clone(),
            title: Some(title),
            message: body,
            priority: Some(priority),
            tags,
            click: None,
            attach: None,
            filename: None,
            delay: None,
            email: None,
            call: None,
            actions: None,
        })
    }

    async fn drain_queue(&self) {
        info!("Draining remaining notification queue");

        while let Ok(task) = self.task_receiver.try_recv() {
            // Decrement queue size when task is dequeued during drain
            self.queue_size.fetch_sub(1, Ordering::Relaxed);
            self.process_task(task).await;
        }
    }
}

// DaemonMessage and DaemonResponse are now imported from shared module



pub fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use std::process::Command;

        // Use kill -0 to check if process exists
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        use std::process::Command;

        // Use tasklist on Windows to check if process exists
        Command::new("tasklist")
            .arg("/FI")
            .arg(format!("PID eq {}", pid))
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
            })
            .unwrap_or(false)
    }
}

