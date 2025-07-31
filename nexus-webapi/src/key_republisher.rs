use std::net::IpAddr;

use nexus_common::types::DynError;
use pkarr::dns::Name;
use pkarr::errors::PublishError;
use pkarr::{dns::rdata::SVCB, SignedPacket};

use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};

use crate::api_context::ApiContext;

/// Republishes the Nexus API's pkarr packet to the DHT every hour.
pub struct NexusApiKeyRepublisher {
    join_handle: JoinHandle<()>,
}

impl NexusApiKeyRepublisher {
    pub async fn start(context: &ApiContext, pubky_tls_port: u16) -> Result<Self, DynError> {
        let signed_packet = create_signed_packet(context, pubky_tls_port)?;
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
            tracing::warn!("Failed to publish the Nexus API's pkarr packet to the DHT: {e}");
        } else {
            tracing::info!("Published the Nexus API's pkarr packet to the DHT.");
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

impl Drop for NexusApiKeyRepublisher {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn create_signed_packet(
    context: &ApiContext,
    local_pubky_tls_port: u16,
) -> Result<SignedPacket, DynError> {
    let root_name: Name = "."
        .try_into()
        .expect(". is the root domain and always valid");

    let mut signed_packet_builder = SignedPacket::builder();

    // TODO Getter / field for public IP
    let public_ip = context.api_config.public_addr.ip();

    let public_pubky_tls_port = local_pubky_tls_port;

    // `SVCB(HTTPS)` record pointing to the pubky tls port and the public ip address
    // This is what is used in all applications expect for browsers.
    let mut svcb = SVCB::new(0, root_name.clone());
    svcb.set_port(public_pubky_tls_port);
    match &public_ip {
        IpAddr::V4(ip) => {
            svcb.set_ipv4hint([ip.to_bits()])?;
        }
        IpAddr::V6(ip) => {
            svcb.set_ipv6hint([ip.to_bits()])?;
        }
    };
    signed_packet_builder = signed_packet_builder.https(root_name.clone(), svcb, 60 * 60);

    // `A` record to the public IP. This is used for regular browser connections.
    signed_packet_builder = signed_packet_builder.address(root_name.clone(), public_ip, 60 * 60);

    Ok(signed_packet_builder.build(&context.keypair)?)
}
