mod proto;
mod snapshot;

use std::{borrow::BorrowMut, cell::RefCell, rc::Rc};

use futures::{future, StreamExt as _};
use tokio::{task, task::spawn_local};

use crate::{
    proto::{Direction, TrackChange},
    snapshot::NegotiationRole,
};

#[tokio::main]
async fn main() {
    task::LocalSet::new()
        .run_until(async {
            let room = Room::new();
            room.peer_created(
                vec![
                    proto::Track {
                        direction: Direction::Send,
                        is_muted: false,
                    },
                    proto::Track {
                        direction: Direction::Recv,
                        is_muted: false,
                    },
                ],
                NegotiationRole::Offerer,
            );

            spawn_local({
                let room = Rc::clone(&room);
                async move {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    room.sdp_answer_made("aaa".to_string());
                }
            });

            room.snapshot
                .peers
                .borrow()
                .iter()
                .next()
                .unwrap()
                .negotiation_role
                .when(|r| r.is_none())
                .await;

            print!("\n\n\n\n");

            room.tracks_applied(
                vec![
                    proto::TrackChange::IceRestart,
                    proto::TrackChange::Update(proto::TrackPatch {
                        is_muted: Some(true),
                    }),
                    proto::TrackChange::Update(proto::TrackPatch {
                        is_muted: Some(true),
                    }),
                ],
                Some(NegotiationRole::Answerer("asdkj".to_string())),
            );

            room.snapshot
                .peers
                .borrow()
                .iter()
                .next()
                .unwrap()
                .negotiation_role
                .when(|r| r.is_none())
                .await;

            print!("\n\n\n\n");

            room.tracks_applied(
                vec![proto::TrackChange::IceRestart],
                Some(NegotiationRole::Answerer("asdkj".to_string())),
            );

            room.snapshot
                .peers
                .borrow()
                .iter()
                .next()
                .unwrap()
                .negotiation_role
                .when(|r| r.is_none())
                .await;
            print!("\n\n\n\n");

            room.tracks_applied(
                vec![
                    proto::TrackChange::Added(proto::Track {
                        is_muted: false,
                        direction: Direction::Recv,
                    }),
                    proto::TrackChange::Added(proto::Track {
                        is_muted: false,
                        direction: Direction::Send,
                    }),
                ],
                Some(NegotiationRole::Answerer("aasd".to_string())),
            );

            room.snapshot
                .peers
                .borrow()
                .iter()
                .next()
                .unwrap()
                .negotiation_role
                .when(|r| r.is_none())
                .await;
            print!("\n\n\n\n");

            room.tracks_applied(
                vec![
                    proto::TrackChange::Added(proto::Track {
                        is_muted: false,
                        direction: Direction::Recv,
                    }),
                    proto::TrackChange::Added(proto::Track {
                        is_muted: false,
                        direction: Direction::Send,
                    }),
                ],
                Some(NegotiationRole::Offerer),
            );
            spawn_local({
                let room = Rc::clone(&room);
                async move {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    room.sdp_answer_made("aaa".to_string());
                }
            });

            room.snapshot
                .peers
                .borrow()
                .iter()
                .next()
                .unwrap()
                .negotiation_role
                .when(|r| r.is_none())
                .await;
        })
        .await;
}

struct Track {
    is_muted: RefCell<bool>,
    direction: Direction,
}

use std::time::Duration;

impl Track {
    fn new(snap: &Rc<snapshot::Track>) -> Self {
        println!("Track [dir = {:?}] created.", snap.direction);
        Self {
            is_muted: RefCell::new(false),
            direction: snap.direction,
        }
    }

    async fn update_is_muted(&self, is_muted: bool) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        println!("Track [dir = {:?}] updated.", self.direction);
        *self.is_muted.borrow_mut() = is_muted;
    }
}

struct Peer {
    tracks: RefCell<Vec<Track>>,
    snapshot: Rc<snapshot::Peer>,
}

impl Peer {
    fn new(snap: &Rc<snapshot::Peer>) -> Self {
        Self {
            tracks: RefCell::default(),
            snapshot: snap.clone(),
        }
    }

    fn spawn_on_track_created(self: Rc<Self>) {

    }

    async fn get_offer(&self) -> String {
        println!("get_offer");
        "sdp_offer".to_string()
    }

    async fn set_remote_offer(&self, remote_offer: String) {
        println!("set_remote_offer");
    }

    async fn restart_ice(&self) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        println!("restart_ice");
    }
}

struct Room {
    snapshot: Rc<snapshot::Room>,
    peers: RefCell<Vec<Rc<Peer>>>,
}

impl Room {
    async fn create_peer(&self, peer_snap: Rc<snapshot::Peer>) {
        let peer = Rc::new(Peer::new(&peer_snap));
        spawn_local({
            let peer = Rc::clone(&peer);
            let peer_snap = Rc::clone(&peer_snap);
            let mut on_track_created = peer_snap.tracks.borrow().on_push();
            async move {
                while let Some(track) = on_track_created.next().await {
                    spawn_local({
                        let peer = Rc::clone(&peer);
                        let peer_snap = Rc::clone(&peer_snap);
                        async move {
                            match track.direction {
                                Direction::Send => {
                                    if let Some(NegotiationRole::Answerer(_)) =
                                        peer_snap
                                            .negotiation_role
                                            .borrow()
                                            .clone()
                                    {
                                        peer_snap
                                            .remote_sdp_offer
                                            .subscribe()
                                            .skip(1)
                                            .filter(|s| {
                                                future::ready(s.is_some())
                                            }).next().await;
                                    }
                                }
                                _ => (),
                            }
                            Self::create_track(
                                peer,
                                peer_snap.clone(),
                                track.clone(),
                            )
                            .await;
                            match track.direction {
                                Direction::Send => {
                                    peer_snap
                                        .senders_update_in_progress
                                        .mutate(|mut c| *c -= 1);
                                }
                                Direction::Recv => {
                                    peer_snap
                                        .receivers_update_in_progress
                                        .mutate(|mut c| *c -= 1);
                                }
                            }
                        }
                    });
                }
            }
        });
        spawn_local({
            let peer = Rc::clone(&peer);
            let peer_snap = Rc::clone(&peer_snap);
            let mut on_negotiation_role = Box::pin(
                peer_snap
                    .negotiation_role
                    .subscribe()
                    .filter_map(|q| future::ready(q)),
            );

            async move {
                while let Some(new_role) = on_negotiation_role.next().await {
                    peer_snap.restart_ice.when_eq(false).await;
                    match new_role {
                        snapshot::NegotiationRole::Offerer => {
                            future::join(
                                peer_snap.senders_update_in_progress.when_eq(0),
                                peer_snap
                                    .receivers_update_in_progress
                                    .when_eq(0),
                            )
                            .await;
                            let sdp_offer = peer.get_offer().await;
                            peer_snap.sdp_offer.set(Some(sdp_offer));
                            peer_snap
                                .remote_sdp_offer
                                .subscribe()
                                .skip(1)
                                .filter_map(|o| future::ready(o))
                                .next()
                                .await;
                            peer_snap.negotiation_finished();
                        }
                        snapshot::NegotiationRole::Answerer(remote_offer) => {
                            peer_snap
                                .receivers_update_in_progress
                                .when_eq(0)
                                .await;
                            peer.set_remote_offer(remote_offer.clone()).await;
                            peer_snap
                                .remote_sdp_offer
                                .set(Some(remote_offer.clone()));
                            peer_snap
                                .senders_update_in_progress
                                .when_eq(0)
                                .await;
                            let sdp_offer = peer.get_offer().await;
                            peer_snap.sdp_offer.set(Some(sdp_offer));
                            peer_snap.negotiation_finished();
                        }
                    }
                }
            }
        });
        spawn_local({
            let peer = Rc::clone(&peer);
            let snap = Rc::clone(&peer_snap);
            let mut on_ice_restart =
                Box::pin(snap.restart_ice.subscribe().filter_map(
                    |is_ice_restart| async move {
                        if is_ice_restart {
                            Some(())
                        } else {
                            None
                        }
                    },
                ));
            async move {
                while let Some(_) = on_ice_restart.next().await {
                    peer.restart_ice().await;
                    snap.restart_ice.set(false);
                }
            }
        });
        self.peers.borrow_mut().push(peer);
    }

    async fn create_track(
        peer: Rc<Peer>,
        peer_snap: Rc<snapshot::Peer>,
        track_snap: Rc<snapshot::Track>,
    ) {
        let track = Rc::new(Track::new(&track_snap));
        spawn_local({
            let track = Rc::clone(&track);
            let mut on_track_muted = track_snap.is_muted.subscribe().skip(1);
            async move {
                while let Some(is_muted) = on_track_muted.next().await {
                    match track_snap.direction {
                        proto::Direction::Send => {
                            peer_snap
                                .receivers_update_in_progress
                                .when_eq(0)
                                .await;
                        }
                        _ => (),
                    }

                    track.update_is_muted(is_muted).await;
                    match track_snap.direction {
                        proto::Direction::Send => {
                            peer_snap
                                .senders_update_in_progress
                                .mutate(|mut c| *c -= 1);
                        }
                        proto::Direction::Recv => {
                            peer_snap
                                .receivers_update_in_progress
                                .mutate(|mut c| *c -= 1);
                        }
                    }
                }
            }
        });
    }
}

impl Room {
    pub fn new() -> Rc<Self> {
        let snapshot = Rc::new(snapshot::Room::new());
        let this = Rc::new(Self {
            snapshot: Rc::clone(&snapshot),
            peers: RefCell::new(Vec::new()),
        });

        let mut on_peer_created = snapshot.peers.borrow().on_push();
        spawn_local({
            let this = Rc::clone(&this);
            async move {
                if let Some(peer) = on_peer_created.next().await {
                    this.create_peer(peer).await;
                }
            }
        });

        this
    }

    fn peer_created(
        &self,
        tracks: Vec<proto::Track>,
        negotiation_role: snapshot::NegotiationRole,
    ) {
        self.snapshot
            .peers
            .borrow_mut()
            .push(snapshot::Peer::new(tracks, negotiation_role));
    }

    fn tracks_applied(
        &self,
        updates: Vec<proto::TrackChange>,
        negotiation_role: Option<NegotiationRole>,
    ) {
        let peer = self.snapshot.peers.borrow().iter().next().unwrap().clone();
        let mut i = 0;
        for update in updates {
            match update {
                proto::TrackChange::IceRestart => {
                    peer.restart_ice.set(true);
                }
                proto::TrackChange::Update(patch) => {
                    let track =
                        peer.tracks.borrow().iter().nth(i).unwrap().clone();

                    let track_transaction =
                        Rc::clone(&track).transaction(Rc::clone(&peer));
                    if let Some(is_muted) = patch.is_muted {
                        track.is_muted.set(is_muted);
                    }

                    i += 1;
                }
                proto::TrackChange::Added(track) => {
                    match track.direction {
                        Direction::Send => peer
                            .senders_update_in_progress
                            .mutate(|mut c| *c += 1),
                        Direction::Recv => peer
                            .receivers_update_in_progress
                            .mutate(|mut c| *c += 1),
                    }
                    peer.tracks.borrow_mut().push(snapshot::Track::new(track));
                }
            }
        }
        peer.negotiation_role.set(negotiation_role);
    }

    fn sdp_answer_made(&self, sdp_answer: String) {
        let peer = self.snapshot.peers.borrow().iter().next().unwrap().clone();
        peer.remote_sdp_offer.set(Some(sdp_answer));
    }
}
