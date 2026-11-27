//! Background task to republish a service's pkarr packet to the DHT.
//!
//! This task should be started on service startup and run until the service is stopped.
//!
//! The task is responsible for:
//! - Republishing the service's pkarr packet to the DHT every hour.
//! - Stopping the task when the service is stopped (dropped).
//!
//! This task is kept generic, with no coupling to Nexus models, such that it may be extracted to a common lib and re-used.

use std::net::IpAddr;

use nexus_common::types::DynError;
use pubky::pkarr::dns::Name;
use pubky::pkarr::errors::PublishError;
use pubky::pkarr::Keypair;
use pubky::pkarr::{self, dns::rdata::SVCB, SignedPacket};
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};

/// Delay before retrying a failed first publish, doubled after each failure up to [MAX_RETRY_DELAY].
const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

/// Information needed to start the KeyRepublisher
pub struct KeyRepublisherContext {
    pub public_ip: IpAddr,
    pub public_pubky_tls_port: u16,

    pub(crate) pkarr_client: pkarr::Client,
    pub(crate) keypair: Keypair,
}

/// Republishes the service's pkarr packet to the DHT periodically.
pub struct KeyRepublisher {
    join_handle: JoinHandle<()>,
}

impl KeyRepublisher {
    pub async fn start(context: &KeyRepublisherContext) -> Result<Self, DynError> {
        let signed_packet = create_signed_packet(context)?;
        let pkarr_client = context.pkarr_client.clone();
        let join_handle = Self::start_periodic_republish(pkarr_client, &signed_packet).await;
        Ok(Self { join_handle })
    }

    #[tracing::instrument(name = "dht.publish", skip_all)]
    async fn publish_once(
        client: &pkarr::Client,
        signed_packet: &SignedPacket,
    ) -> Result<(), PublishError> {
        match client.publish(signed_packet).await {
            Ok(stored_on) => {
                tracing::info!(
                    stored_on,
                    "Published the service's pkarr packet to the DHT."
                );
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to publish the service's pkarr packet to the DHT: {e}");
                Err(e)
            }
        }
    }

    /// Start the periodic republish task which will republish the server packet to the DHT every hour.
    ///
    /// The first publish is awaited, so the packet is on the DHT when this returns if the DHT
    /// answers. A failed first publish is not fatal (#1163): the task retries it with backoff
    /// until it succeeds, then republishes every hour.
    async fn start_periodic_republish(
        client: pkarr::Client,
        signed_packet: &SignedPacket,
    ) -> JoinHandle<()> {
        let published = needs_no_retry(&Self::publish_once(&client, signed_packet).await);

        // Start the periodic republish task.
        let signed_packet = signed_packet.clone();
        tokio::spawn(republish(published, move || {
            let (client, signed_packet) = (client.clone(), signed_packet.clone());
            async move { needs_no_retry(&Self::publish_once(&client, &signed_packet).await) }
        }))
    }

    /// Stop the periodic republish task.
    pub fn stop(&self) {
        self.join_handle.abort();
    }
}

impl Drop for KeyRepublisher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Whether a publish result ends the retries. `NotMostRecent` means the DHT holds a newer
/// packet for this key (another instance with the same secret): retrying cannot fix it.
fn needs_no_retry(result: &Result<(), PublishError>) -> bool {
    matches!(result, Ok(()) | Err(PublishError::NotMostRecent))
}

/// Calls `publish` until it succeeds, unless `published`: first after [INITIAL_RETRY_DELAY], then
/// doubling the delay up to [MAX_RETRY_DELAY]. Then calls it once every hour.
async fn republish<F, Fut>(published: bool, mut publish: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    if !published {
        let mut delay = INITIAL_RETRY_DELAY;
        loop {
            tokio::time::sleep(delay).await;
            if publish().await {
                break;
            }
            delay = (delay * 2).min(MAX_RETRY_DELAY);
        }
    }

    let mut interval = interval(Duration::from_secs(60 * 60)); // 1 hour in seconds
    interval.tick().await; // This ticks immediatly. Wait for first interval before starting the loop.
    loop {
        interval.tick().await;
        publish().await;
    }
}

fn create_signed_packet(context: &KeyRepublisherContext) -> Result<SignedPacket, DynError> {
    let root_name: Name = "."
        .try_into()
        .expect(". is the root domain and always valid");

    let public_ip = context.public_ip;
    let public_pubky_tls_port = context.public_pubky_tls_port;
    let ttl_sec = 60 * 60 * 3; // 3h

    let mut signed_packet_builder = SignedPacket::builder();

    // `SVCB(HTTPS)` record pointing to the pubky tls port and the public ip address
    // This is what is used in all applications expect for browsers.
    let mut svcb = SVCB::new(1, root_name.clone());
    svcb.set_port(public_pubky_tls_port);
    match &public_ip {
        IpAddr::V4(ip) => svcb.set_ipv4hint(&[ip.to_bits()]),
        IpAddr::V6(ip) => svcb.set_ipv6hint(&[ip.to_bits()]),
    };
    signed_packet_builder = signed_packet_builder.https(root_name.clone(), svcb, ttl_sec);

    // `A` record to the public IP. This is used for regular browser connections.
    signed_packet_builder = signed_packet_builder.address(root_name.clone(), public_ip, ttl_sec);

    Ok(signed_packet_builder.build(&context.keypair)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #1163: a first publish that fails does not stop the service from starting, and the
    /// task keeps running to retry it.
    #[tokio::test]
    async fn a_failed_first_publish_still_starts_the_republisher() {
        let pkarr_client = pkarr::Client::builder()
            .no_default_network()
            .bootstrap(&["127.0.0.1:1"])
            .build()
            .unwrap();
        let context = KeyRepublisherContext {
            public_ip: "127.0.0.1".parse().unwrap(),
            public_pubky_tls_port: 4433,
            pkarr_client,
            keypair: Keypair::random(),
        };

        let republisher = KeyRepublisher::start(&context)
            .await
            .expect("a failed publish is retried, not returned");

        assert!(!republisher.join_handle.is_finished());
    }

    /// A failed first publish is retried after 1 s, 2 s, 4 s…, and once one succeeds the
    /// packet is republished every hour.
    #[tokio::test(start_paused = true)]
    async fn a_failed_first_publish_is_retried_with_backoff_then_hourly() {
        use std::sync::{Arc, Mutex};

        let start = tokio::time::Instant::now();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut results = [false, false, true].into_iter();
        let recorded = calls.clone();
        let task = tokio::spawn(republish(false, move || {
            recorded.lock().unwrap().push(start.elapsed().as_secs());
            let ok = results.next().unwrap_or(true);
            async move { ok }
        }));

        tokio::time::sleep(Duration::from_secs(7 + 2 * 3600 + 1)).await;
        task.abort();

        assert_eq!(*calls.lock().unwrap(), [1, 3, 7, 7 + 3600, 7 + 2 * 3600]);
    }

    #[test]
    fn a_newer_packet_on_the_dht_ends_the_retries() {
        assert!(needs_no_retry(&Ok(())));
        assert!(needs_no_retry(&Err(PublishError::NotMostRecent)));
        assert!(!needs_no_retry(&Err(PublishError::NoResponses)));
    }
}
