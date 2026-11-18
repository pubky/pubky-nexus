pub mod common;
mod homeserver_client;
mod processor;
mod processor_runner;
mod result;
mod setup;

pub use common::{create_mock_handler, new_in_memory_store, TEST_USER_ID};
pub use homeserver_client::MockHomeserverClient;
pub use processor::{
    create_mock_event_processors, create_random_homeservers_and_persist, MockEventProcessor,
};
pub use processor_runner::MockEventProcessorRunner;
pub use result::MockEventProcessorResult;
pub use setup::{setup, HS_IDS};

use pubky_app_specs::PubkyId;

pub fn hs_id(s: &str) -> PubkyId {
    PubkyId::try_from(s).expect("valid PubkyId")
}
