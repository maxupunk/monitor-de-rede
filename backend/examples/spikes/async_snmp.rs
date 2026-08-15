//! Spike reproduzível da decisão registrada na Fase 9.
//!
//! Uso: `cargo run --example spike_async_snmp -- <host:porta> <community>`.

use std::time::Duration;

use async_snmp::{Auth, ClientBuilder, Oid, OidOrdering, Retry, UdpTransport, WalkMode};
use futures::TryStreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:161".into());
    let community = std::env::args().nth(2).unwrap_or_else(|| "public".into());
    let transport = UdpTransport::bind("0.0.0.0:0").await?;
    let client = ClientBuilder::new(target, Auth::v2c(community))
        .timeout(Duration::from_secs(1))
        .retry(Retry::exponential(2).jitter(0.25))
        .max_repetitions(20)
        .walk_mode(WalkMode::Auto)
        .oid_ordering(OidOrdering::Strict)
        .max_walk_results(20_000)
        .build_with(&transport)
        .await?;
    let oid = "1.3.6.1.2.1.1".parse::<Oid>()?;
    let values = client.walk(oid)?.try_collect::<Vec<_>>().await?;
    for value in values {
        println!("{value}");
    }
    Ok(())
}
