use log::{debug, error, info};
use once_cell::sync::OnceCell;
use std::sync::{Arc, Weak};
use tokio::sync::mpsc;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};

use super::event::RetryEvent;

pub const CHANNEL_BUFFER: usize = 1024;
pub static RETRY_MANAGER: OnceCell<RetryManager> = OnceCell::new();

/// Represents commands sent to the retry manager for handling events in the retry queue
#[derive(Debug, Clone)]
pub enum RetryQueueMessage {
    // Command to retry all pending events that are waiting to be indexed
    RetryEvent(String),
    /// Command to process a specific event in the retry queue
    /// This action can either add an event to the queue or use the event as context to remove it from the queue
    /// - `String`: The index or identifier of the event being processed
    /// - `RetryEvent`: The event to be added to or processed for removal from the retry queue
    ProcessEvent(String, RetryEvent),
}

pub type WeakRetryManagerSenderChannel = Weak<Mutex<Option<Sender<RetryQueueMessage>>>>;
//type RetryManagerReceiverChannel = Receiver<RetryQueueMessage>;

/// Manages the retry queue for processing failed events.
///
/// The `RetryManager` is responsible for handling messages that involve retrying or
/// processing failed events in an asynchronous event system. It listens for incoming
/// retry-related messages and processes them accordingly
///
/// ## Responsibilities:
/// - Receives and processes messages related to event retries
/// - Maintains a queue of events that need to be retried
/// - Ensures failed events are reprocessed in an orderly manner
#[derive(Debug, Clone)]
pub struct RetryManager {
    sender_channel: Arc<Mutex<Option<Sender<RetryQueueMessage>>>>,
}

/// Initializes a new `RetryManager` with a message-passing channel
impl RetryManager {
    pub fn initialise() -> &'static RetryManager {
        RETRY_MANAGER.get_or_init(|| {
            let (tx, rx) = mpsc::channel(CHANNEL_BUFFER);

            tokio::spawn(Self::process_messages(rx));

            RetryManager {
                sender_channel: Arc::new(Mutex::new(Some(tx))),
            }
        })
    }

    /// Returns a weak reference to the sender channel
    ///
    /// It clones the sender channel as a `Weak` reference instead of `Arc`,
    /// preventing strong ownership cycles that could lead to memory leaks. By using `Weak`,
    /// the sender channel can be automatically cleaned up when all strong references
    /// (`Arc`) to it are dropped. This ensures that:
    ///
    /// - The sender channel is **not kept alive indefinitely** if the `RetryManager` is restarted
    /// - It allows us to **check if the sender is still valid** before attempting to send messages
    /// - Prevents **unnecessary memory usage** by ensuring unused channels are cleaned up
    ///
    /// # Returns:
    /// - A `Weak` reference to the sender channel, which can be upgraded to `Arc` when needed
    pub fn clone_sender_channel() -> Weak<Mutex<Option<Sender<RetryQueueMessage>>>> {
        Arc::downgrade(&Self::initialise().sender_channel)
    }

    /// Replaces the existing sender channel with a new one and restarts the communication channel
    pub async fn restart() {
        let (new_sender_channel, new_receiver_channel) = mpsc::channel(CHANNEL_BUFFER);

        {
            let mut sender_lock = Self::initialise().sender_channel.lock().await;
            *sender_lock = None; // Remove old sender
        }

        tokio::spawn(RetryManager::process_messages(new_receiver_channel));

        let mut sender_lock = Self::initialise().sender_channel.lock().await;
        *sender_lock = Some(new_sender_channel.clone());

        info!("RetryManager restarted the channel communication");
    }

    /// Executes the main event loop to process messages from the channel
    /// This function listens for incoming messages on the receiver channel and handles them
    /// based on their type
    /// The loop runs continuously until the channel is closed, ensuring that all messages
    /// are processed appropriately
    ///
    ///  # Arguments
    /// * `rx` - The receiver channel for retry messages
    async fn process_messages(mut rx: Receiver<RetryQueueMessage>) {
        // Listen all the messages in the channel
        while let Some(message) = rx.recv().await {
            match message {
                RetryQueueMessage::ProcessEvent(index_key, retry_event) => {
                    error!("{}, {}", retry_event.error_type, index_key);
                    if let Err(err) = retry_event.put_to_index(index_key).await {
                        error!("Failed to put event to index: {}", err);
                    }
                }
                _ => debug!("New message received, not handled: {:?}", message),
            }
        }
    }
}
