use std::{
    cell::RefCell,
    net::SocketAddr,
    rc::Rc,
    time::Instant,
};

use neqo_http3::{Http3Client, Http3Parameters, Http3State};
use neqo_transport::EmptyConnectionIdGenerator;

pub struct NeqoHttp3Core {
    client: Http3Client,
}

impl NeqoHttp3Core {
    pub fn new(
        server_name: &str,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    ) -> Result<Self, String> {
        nss::init().map_err(|error| format!("NSS init failed: {error}"))?;
        let client = Http3Client::new(
            server_name,
            Rc::new(RefCell::new(EmptyConnectionIdGenerator::default())),
            local_addr,
            remote_addr,
            Http3Parameters::default(),
            Instant::now(),
        )
        .map_err(|error| format!("Neqo HTTP/3 init failed: {error}"))?;
        Ok(Self { client })
    }

    pub fn state(&self) -> Http3State {
        self.client.state()
    }

    /// Drive Neqo far enough to produce a real QUIC Initial datagram.
    /// The caller owns UDP I/O and certificate decisions.
    pub fn initial_datagram(&mut self) -> Option<Vec<u8>> {
        self.client
            .process_output(Instant::now())
            .dgram()
            .map(|datagram| datagram.to_vec())
    }

    pub fn transport_stats_debug(&self) -> String {
        format!("{:?}", self.client.transport_stats())
    }
}

pub fn default_probe() -> Result<NeqoHttp3Core, String> {
    let local = "0.0.0.0:0"
        .parse::<SocketAddr>()
        .map_err(|error| error.to_string())?;
    let remote = "192.0.2.1:443"
        .parse::<SocketAddr>()
        .map_err(|error| error.to_string())?;
    NeqoHttp3Core::new("example.com", local, remote)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_real_neqo_http3_client() {
        let probe = default_probe().expect("create Neqo client");
        assert!(matches!(probe.state(), Http3State::Initializing));
    }

    #[test]
    fn emits_real_quic_initial_packet() {
        let mut probe = default_probe().expect("create Neqo client");
        let datagram = probe.initial_datagram().expect("QUIC initial datagram");
        assert!(datagram.len() >= 1200, "QUIC Initial must be padded");
        assert_ne!(datagram[0] & 0x80, 0, "QUIC Initial uses a long header");
    }
}
