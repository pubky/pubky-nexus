use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::error::EventProcessorError;
use super::retry::manager::{RetryQueueMessage, WeakRetryManagerSenderChannel};
use super::Event;
use crate::events::retry::event::RetryEvent;
use crate::events::retry::manager::RetryManager;
use crate::types::DynError;
use crate::types::PubkyId;
use crate::PubkyConnector;
use crate::{models::homeserver::Homeserver, Config};
use log::{debug, error, info, warn};
use tokio::{sync::mpsc, time::Duration};

// REVIEW: This could be env variables and can be part of EventProcessor
const MAX_ATTEMPTS: i32 = 5;
const MESSAGE_SEND_RETRY: u64 = 10;
const RETRY_THRESHOLD: usize = 20;

pub struct EventProcessor {
    pub homeserver: Homeserver,
    limit: u32,
    pub retry_manager_sender_channel: WeakRetryManagerSenderChannel,
    consecutive_message_send_failures: Arc<AtomicUsize>,
}

impl EventProcessor {
    pub fn set_sender_channel(&mut self, channel: WeakRetryManagerSenderChannel) {
        self.retry_manager_sender_channel = channel;
    }
    /// Creates a new `EventProcessor` instance for testing purposes.
    ///
    /// This function initializes an `EventProcessor` configured with:
    /// - A mock homeserver constructed using the provided `homeserver_url` and `homeserver_pubky`.
    /// - A default configuration, including an HTTP client, a limit of 1000 events, and a sender channel.
    ///
    /// It is designed for use in integration tests, benchmarking scenarios, or other test environments
    /// where a controlled and predictable `EventProcessor` instance is required.
    ///
    /// # Parameters
    /// - `homeserver_id`: A `String` representing the URL of the homeserver to be used in the test environment.
    /// - `tx`: A `RetryManagerSenderChannel` used to handle outgoing messages or events.
    pub async fn test(
        homeserver_id: String,
        retry_manager_sender_channel: WeakRetryManagerSenderChannel,
    ) -> Self {
        let id = PubkyId(homeserver_id.to_string());
        let homeserver = Homeserver::new(id).await.unwrap();
        Self {
            homeserver,
            limit: 1000,
            retry_manager_sender_channel,
            consecutive_message_send_failures: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub async fn from_config(
        config: &Config,
        retry_manager_sender_channel: WeakRetryManagerSenderChannel,
    ) -> Result<Self, DynError> {
        let homeserver = Homeserver::from_config(config).await?;
        let limit = config.events_limit;

        info!(
            "Initialized Event Processor for homeserver: {:?}",
            homeserver
        );

        Ok(Self {
            homeserver,
            limit,
            retry_manager_sender_channel,
            consecutive_message_send_failures: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub async fn run(&mut self) -> Result<(), DynError> {
        let lines = { self.poll_events().await.unwrap_or_default() };
        if let Some(lines) = lines {
            self.process_event_lines(lines).await?;
        };
        Ok(())
    }

    /// Polls new events from the homeserver.
    ///
    /// It sends a GET request to the homeserver's events endpoint
    /// using the current cursor and a specified limit. It retrieves new event
    /// URIs in a newline-separated format, processes it into a vector of strings,
    /// and returns the result.
    async fn poll_events(&mut self) -> Result<Option<Vec<String>>, DynError> {
        debug!("Polling new events from homeserver");

        let response: String;
        {
            let pubky_client = PubkyConnector::get_pubky_client()?;
            response = pubky_client
                .get(format!(
                    "https://{}/events/?cursor={}&limit={}",
                    self.homeserver.id, self.homeserver.cursor, self.limit
                ))
                .send()
                .await?
                .text()
                .await?;
        }

        let lines: Vec<String> = response.trim().split('\n').map(|s| s.to_string()).collect();
        debug!("Homeserver response lines {:?}", lines);

        if lines.len() == 1 && lines[0].is_empty() {
            info!("No new events");
            Ok(None)
        } else {
            Ok(Some(lines))
        }
    }

    /// Processes a batch of event lines retrieved from the homeserver.
    ///
    /// This function iterates over a vector of event URIs, handling each line based on its content:
    /// - Lines starting with `cursor:` update the cursor for the homeserver and save it to the index.
    /// - Other lines are parsed into events and processed accordingly. If parsing fails, an error is logged.
    ///
    /// # Parameters
    /// - `lines`: A vector of strings representing event lines retrieved from the homeserver.
    pub async fn process_event_lines(&mut self, lines: Vec<String>) -> Result<(), DynError> {
        for line in &lines {
            if line.starts_with("cursor:") {
                if let Some(cursor) = line.strip_prefix("cursor: ") {
                    self.homeserver.cursor = cursor.to_string();
                    self.homeserver.put_to_index().await?;
                    info!("Cursor for the next request: {}", cursor);
                }
            } else {
                let event = match Event::parse_event(line) {
                    Ok(event) => event,
                    Err(e) => {
                        error!("{}", e);
                        None
                    }
                };
                if let Some(event) = event {
                    debug!("Processing event: {:?}", event);
                    self.handle_event(event).await?;
                }
            }
        }

        Ok(())
    }

    /// Processes an event and track the fail event it if necessary
    /// # Parameters:
    /// - `event`: The event to be processed
    async fn handle_event(&mut self, event: Event) -> Result<(), DynError> {
        let mut attempts = 0;
        let mut delay = Duration::from_millis(MESSAGE_SEND_RETRY);

        if let Err(e) = event.clone().handle().await {
            if let Some((index_key, retry_event)) = extract_retry_event_info(&event, e) {
                let queue_message = RetryQueueMessage::ProcessEvent(index_key.clone(), retry_event);

                while attempts < MAX_ATTEMPTS {
                    let mut restart = false;
                    if let Some(sender_arc) = self.retry_manager_sender_channel.upgrade() {
                        let mut sender_guard = sender_arc.lock().await;

                        if let Some(sender) = sender_guard.as_mut() {
                            match sender.try_send(queue_message.clone()) {
                                Err(mpsc::error::TrySendError::Full(_)) => {
                                    warn!(
                                        "Retry channel is full. Retrying in {:?}... (attempt {}/{})",
                                        delay, attempts + 1, MAX_ATTEMPTS
                                    );
                                }
                                Err(mpsc::error::TrySendError::Closed(_)) => {
                                    warn!("Retry channel receiver is unavailable! Restarting RetryManager...");
                                    restart = true;
                                }
                                _ => {
                                    // Message sent successfully, reset failure counter
                                    self.consecutive_message_send_failures
                                        .store(0, Ordering::Relaxed);
                                    return Ok(());
                                }
                            }
                        } else {
                            warn!("Retry sender channel has been dropped. Retrying in {:?}... (attempt {}/{})",
                                delay, attempts + 1, MAX_ATTEMPTS
                            );
                        }
                    } else {
                        warn!("Sender is invalid/dropped. Retrying event in next iteration...");
                    }
                        
                    if restart {
                        self.update_sender_channel().await;
                        break;
                    }
                    
                    tokio::time::sleep(delay).await;
                    // Apply exponential backoff before the next retry attempt
                    delay *= 2;
                    attempts += 1;
                }

                error!(
                    "Message passing failed: Unable to send event {:?} after {} attempts. Dropping event...",
                    index_key, MAX_ATTEMPTS
                );

                // Increase failure counter to track consecutive failures
                self.consecutive_message_send_failures
                    .fetch_add(1, Ordering::Relaxed);
                self.evaluate_message_send_failures().await;
            }
        }
        Ok(())
    }

    /// Checks if the consecutive message send failure count has reached RETRY_THRESHOLD
    /// and triggers a RetryManager restart if necessary
    /// This prevents RetryManager from restarting too frequently
    async fn evaluate_message_send_failures(&mut self) {
        // Before reset the retry manager, reset the consecutive message send fail counter
        let reset_needed = self
            .consecutive_message_send_failures
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                if count >= RETRY_THRESHOLD {
                    Some(0) // Reset counter
                } else {
                    None
                }
            })
            .is_ok();

        if reset_needed {
            warn!(
                "Message send failure threshold reached ({} consecutive failures). Restarting RetryManager...",
                RETRY_THRESHOLD
            );
            self.update_sender_channel().await;
        }
    }

    /// Restarts the RetryManager and updates the sender channel reference
    async fn update_sender_channel(&mut self) {
        RetryManager::restart().await;
        self.set_sender_channel(RetryManager::clone_sender_channel());
    }
}

/// Extracts retry-related information from an event and its associated error
///
/// # Parameters
/// - `event`: Reference to the event for which retry information is being extracted
/// - `error`: Determines whether the event is eligible for a retry or should be discarded
fn extract_retry_event_info(event: &Event, error: DynError) -> Option<(String, RetryEvent)> {
    let retry_event = match error.downcast_ref::<EventProcessorError>() {
        Some(EventProcessorError::InvalidEventLine { message }) => {
            error!("{}", message);
            return None;
        }
        Some(event_processor_error) => RetryEvent::new(event_processor_error.clone()),
        // Others errors must be logged at least for now
        None => {
            error!("Unhandled error type for URI: {}, {:?}", event.uri, error);
            return None;
        }
    };

    // Generate a compress index to save in the cache
    let index = match RetryEvent::generate_index_key(&event.uri) {
        Some(retry_index) => retry_index,
        None => {
            return None;
        }
    };
    Some((format!("{}:{}", event.event_type, index), retry_event))
}
