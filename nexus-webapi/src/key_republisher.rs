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
        let published = Self::publish_once(&client, signed_packet).await.is_ok();

        // Start the periodic republish task.
        let signed_packet = signed_packet.clone();
        tokio::spawn(async move {
            if !published {
                let mut delay = INITIAL_RETRY_DELAY;
                loop {
                    tokio::time::sleep(delay).await;
                    if Self::publish_once(&client, &signed_packet).await.is_ok() {
                        break;
                    }
                    delay = (delay * 2).min(MAX_RETRY_DELAY);
                }
            }

            let mut interval = interval(Duration::from_secs(60 * 60)); // 1 hour in seconds
            interval.tick().await; // This ticks immediatly. Wait for first interval before starting the loop.
            loop {
                interval.tick().await;
                let _ = Self::publish_once(&client, &signed_packet).await;
            }
        })
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
}
