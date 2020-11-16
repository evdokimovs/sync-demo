use std::{cell::RefCell, rc::Rc};

use futures::StreamExt as _;
use medea_reactive::ObservableVec;
use tokio::task::spawn_local;

use crate::{component, sys::RtcPeerConnection};

pub struct Room {
    pub peers: RefCell<ObservableVec<Rc<component::Peer>>>,
}

impl Room {
    pub fn new() -> Self {
        Self {
            peers: RefCell::new(ObservableVec::new()),
        }
    }

    pub fn spawn_tasks(&self) {
        self.spawn_on_peer_created();
    }

    pub fn spawn_on_peer_created(&self) {
        let mut on_peer_created = self.peers.borrow().on_push();
        spawn_local(async move {
            while let Some(peer) = on_peer_created.next().await {
                let rtc_peer = RtcPeerConnection::new();
                Rc::clone(&peer).spawn_tasks(rtc_peer);
            }
        });
    }
}
