use std::time::Duration;

use tokio::time::sleep;

#[derive(Clone, Debug)]
pub struct MediaStreamTrack;

impl MediaStreamTrack {
    pub fn new() -> Self {
        Self
    }

    pub async fn set_enabled(&self, _: bool) {
        println!("Set enabled");
        sleep(Duration::from_millis(500)).await;
    }
}
