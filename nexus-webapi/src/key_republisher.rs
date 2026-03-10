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
        let join_handle = Self::start_periodic_republish(pkarr_client, &signed_packet).await?;
        Ok(Self { join_handle })
    }

    async fn publish_once(
        client: &pkarr::Client,
        signed_packet: &SignedPacket,
    ) -> Result<(), PublishError> {
        let res = client.publish(signed_packet, None).await;
        if let Err(e) = &res {
            tracing::warn!("Failed to publish the service's pkarr packet to the DHT: {e}");
        } else {
            tracing::info!("Published the service's pkarr packet to the DHT.");
        }
        res
    }

    /// Start the periodic republish task which will republish the server packet to the DHT every hour.
    ///
    /// # Errors
    /// - Throws an error if the initial publish fails.
    /// - Throws an error if the periodic republish task is already running.
    async fn start_periodic_republish(
        client: pkarr::Client,
        signed_packet: &SignedPacket,
    ) -> Result<JoinHandle<()>, DynError> {
        // Publish once to make sure the packet is published to the DHT before this
        // function returns.
        // Throws an error if the packet is not published to the DHT.
        Self::publish_once(&client, signed_packet).await?;

        // Start the periodic republish task.
        let signed_packet = signed_packet.clone();
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60 * 60)); // 1 hour in seconds
            interval.tick().await; // This ticks immediatly. Wait for first interval before starting the loop.
            loop {
                interval.tick().await;
                let _ = Self::publish_once(&client, &signed_packet).await;
            }
        });

        Ok(handle)
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
        IpAddr::V4(ip) => svcb.set_ipv4hint([ip.to_bits()])?,
        IpAddr::V6(ip) => svcb.set_ipv6hint([ip.to_bits()])?,
    };
    signed_packet_builder = signed_packet_builder.https(root_name.clone(), svcb, ttl_sec);

    // `A` record to the public IP. This is used for regular browser connections.
    signed_packet_builder = signed_packet_builder.address(root_name.clone(), public_ip, ttl_sec);

    Ok(signed_packet_builder.build(&context.keypair)?)
}
