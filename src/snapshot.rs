use super::proto;
use crate::{proto::Direction, MediaStreamTrack, RtcPeerConnection};
use futures::{future, StreamExt as _};
use medea_reactive::{ObservableCell, ObservableVec, ProgressableObservable};
use std::{cell::RefCell, rc::Rc};
use tokio::task::spawn_local;

pub struct Room {
    pub peers: RefCell<ObservableVec<Rc<Peer>>>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NegotiationState {
    HaveRemote,
    HaveLocal,
    Stable,
}

pub struct Peer {
    pub senders: RefCell<ObservableVec<Rc<Sender>>>,
    pub receivers: RefCell<ObservableVec<Rc<Receiver>>>,
    pub restart_ice: RefCell<ProgressableObservable<bool>>,
    pub negotiation_role: ObservableCell<Option<NegotiationRole>>,
    pub negotiation_state: ObservableCell<NegotiationState>,
    pub remote_sdp_offer: ObservableCell<Option<String>>,
}

impl Peer {
    pub fn new(
        tracks: Vec<proto::Track>,
        negotiation_role: NegotiationRole,
    ) -> Rc<Self> {
        let mut senders = Vec::new();
        let mut receivers = Vec::new();
        for track in tracks {
            match track.direction {
                Direction::Send => {
                    senders.push(Sender::new(track.id, track.is_muted));
                }
                Direction::Recv => {
                    receivers.push(Receiver::new(track.id, track.is_muted));
                }
            }
        }

        Rc::new(Self {
            senders: RefCell::new(senders.into()),
            receivers: RefCell::new(receivers.into()),
            negotiation_role: ObservableCell::new(Some(negotiation_role)),
            negotiation_state: ObservableCell::new(NegotiationState::Stable),
            restart_ice: RefCell::new(ProgressableObservable::new(false)),
            remote_sdp_offer: ObservableCell::new(None),
        })
    }

    pub fn spawn_tasks(self: Rc<Self>, peer: RtcPeerConnection) {
        Rc::clone(&self).spawn_on_receiver_added(peer.clone());
        Rc::clone(&self).spawn_on_sender_added(peer.clone());
        Rc::clone(&self).spawn_on_negotiation_needed(peer.clone());
        Rc::clone(&self).spawn_on_remote_offer(peer.clone());
        Rc::clone(&self).spawn_on_ice_restart(peer.clone());
    }

    fn spawn_on_remote_offer(self: Rc<Self>, peer: RtcPeerConnection) {
        let mut on_remote_offer = self.remote_sdp_offer.subscribe();
        spawn_local(async move {
            while let Some(remote_offer) = on_remote_offer.next().await {
                if let Some(remote_offer) = remote_offer {
                    peer.set_remote_offer(remote_offer).await;

                    let new_state = match self.negotiation_state.get() {
                        NegotiationState::Stable => {
                            NegotiationState::HaveRemote
                        }
                        NegotiationState::HaveLocal => NegotiationState::Stable,
                        NegotiationState::HaveRemote => {
                            NegotiationState::HaveRemote
                        }
                    };
                    self.negotiation_state.set(new_state);
                }
            }
        });
    }

    fn spawn_on_sender_added(self: Rc<Self>, peer: RtcPeerConnection) {
        let mut on_sender_added = self.senders.borrow().on_push();
        spawn_local(async move {
            while let Some(sender) = on_sender_added.next().await {
                if let Some(NegotiationRole::Answerer(_)) =
                    self.negotiation_role.borrow().clone()
                {
                    if let Err(_) = self
                        .negotiation_state
                        .when(|s| *s == NegotiationState::HaveRemote)
                        .await
                    {
                        break;
                    }
                }
                peer.add_transceiver().await;
                Rc::clone(&sender).spawn_tasks(MediaStreamTrack::new());
                println!("Sender created");
            }
        });
    }

    fn spawn_on_receiver_added(self: Rc<Self>, peer: RtcPeerConnection) {
        let mut on_receiver_added = self.receivers.borrow().on_push();
        spawn_local(async move {
            while let Some(receiver) = on_receiver_added.next().await {
                peer.add_transceiver().await;
                Rc::clone(&receiver).spawn_tasks(MediaStreamTrack::new());
                println!("Receiver created");
            }
        });
    }

    fn spawn_on_ice_restart(self: Rc<Self>, peer: RtcPeerConnection) {
        let mut on_ice_restart = self.restart_ice.borrow().subscribe();
        spawn_local(async move {
            while let Some(ice_restart) = on_ice_restart.next().await {
                if *ice_restart {
                    peer.restart_ice().await;
                }
                *self.restart_ice.borrow_mut().borrow_mut() = false;
            }
        });
    }

    fn spawn_on_negotiation_needed(self: Rc<Self>, peer: RtcPeerConnection) {
        let mut on_negotiation_needed = self.negotiation_role.subscribe();
        spawn_local(async move {
            while let Some(negotiation_needed) =
                on_negotiation_needed.next().await
            {
                let wait_for_ice_restart = {
                    let wait_for_ice_restart = self.restart_ice.borrow();
                    wait_for_ice_restart.when_all_processed()
                };
                match negotiation_needed {
                    Some(NegotiationRole::Offerer) => {
                        future::join(
                            self.receivers.borrow().when_push_completed(),
                            self.senders.borrow().when_push_completed(),
                        )
                        .await;
                        wait_for_ice_restart.await;
                        peer.get_local_offer().await;
                        self.negotiation_state.set(NegotiationState::HaveLocal);

                        if let Err(_) = self
                            .negotiation_state
                            .when_eq(NegotiationState::Stable)
                            .await
                        {
                            break;
                        }
                        self.negotiation_role.set(None);
                    }
                    Some(NegotiationRole::Answerer(remote_offer)) => {
                        self.receivers.borrow().when_push_completed().await;
                        self.remote_sdp_offer.set(Some(remote_offer));

                        self.senders.borrow().when_push_completed().await;

                        wait_for_ice_restart.await;
                        peer.get_local_offer().await;
                        self.negotiation_state.set(NegotiationState::Stable);
                        self.negotiation_role.set(None);
                    }
                    _ => (),
                }
            }
        });
    }
}

pub struct Sender {
    pub id: u32,
    pub is_muted: RefCell<ProgressableObservable<bool>>,
}

impl Sender {
    pub fn new(id: u32, is_muted: bool) -> Rc<Self> {
        Rc::new(Self {
            id,
            is_muted: RefCell::new(ProgressableObservable::new(is_muted)),
        })
    }

    pub fn spawn_tasks(self: Rc<Self>, track: MediaStreamTrack) {
        Rc::clone(&self).spawn_on_muted(track);
    }

    fn spawn_on_muted(self: Rc<Self>, track: MediaStreamTrack) {
        let mut on_muted = self.is_muted.borrow().subscribe();
        spawn_local(async move {
            while let Some(is_muted) = on_muted.next().await {
                track.set_enabled(!*is_muted).await;
            }
        });
    }
}

pub struct Receiver {
    pub id: u32,
    pub is_muted: RefCell<ProgressableObservable<bool>>,
}

impl Receiver {
    pub fn new(id: u32, is_muted: bool) -> Rc<Self> {
        Rc::new(Self {
            id,
            is_muted: RefCell::new(ProgressableObservable::new(is_muted)),
        })
    }

    pub fn spawn_tasks(self: Rc<Self>, track: MediaStreamTrack) {
        Rc::clone(&self).spawn_on_muted(track);
    }

    fn spawn_on_muted(self: Rc<Self>, track: MediaStreamTrack) {
        let mut on_muted = self.is_muted.borrow().subscribe();
        spawn_local(async move {
            while let Some(is_muted) = on_muted.next().await {
                track.set_enabled(!*is_muted).await;
            }
        });
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NegotiationRole {
    Offerer,
    Answerer(String),
}
