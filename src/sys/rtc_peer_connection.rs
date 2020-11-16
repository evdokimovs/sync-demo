use std::time::Duration;

use tokio::time::sleep;

#[derive(Clone)]
pub struct RtcPeerConnection;

impl RtcPeerConnection {
    pub fn new() -> Self {
        RtcPeerConnection
    }

    pub async fn restart_ice(&self) {
        println!("Ice restart");
        sleep(Duration::from_millis(500)).await;
    }

    pub async fn set_remote_offer(&self, _: String) {
        println!("Set remote offer");
        sleep(Duration::from_millis(500)).await;
    }

    pub async fn get_local_offer(&self) -> String {
        println!("Get local offer");
        sleep(Duration::from_millis(500)).await;

        "SDP OFFER".to_string()
    }

    pub async fn add_transceiver(&self) {
        println!("Add transceiver");
        sleep(Duration::from_millis(500)).await;
    }
}
