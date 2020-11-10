mod proto;
mod snapshot;

use std::{borrow::BorrowMut, cell::RefCell, rc::Rc, time::Duration};

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
    snapshot: Rc<snapshot::Track>,
    peer_snapshot: Rc<snapshot::Peer>,
}

impl Track {
    fn new(
        snapshot: Rc<snapshot::Track>,
        peer_snapshot: Rc<snapshot::Peer>,
    ) -> Rc<Self> {
        println!("Track [dir = {:?}] created.", snapshot.direction);
        let this = Rc::new(Self {
            is_muted: RefCell::new(false),
            snapshot,
            peer_snapshot,
        });

        Rc::clone(&this).spawn_on_track_muted();

        this
    }

    /// Spawns listener for the mute/unmute of the [`Track`].
    fn spawn_on_track_muted(self: Rc<Self>) {
        // skip initial is_muted value, because we don't need it in the real app
        let mut on_track_muted = self.snapshot.is_muted.subscribe().skip(1);
        spawn_local({
            async move {
                while let Some(is_muted) = on_track_muted.next().await {
                    match self.snapshot.direction {
                        proto::Direction::Send => {
                            // wait until all receivers will be updated, before
                            // updating this track
                            self.peer_snapshot
                                .receivers_update_in_progress
                                .when_eq(0)
                                .await;
                        }
                        _ => (),
                    }

                    self.update_is_muted(is_muted).await;

                    // decrease track update schedules counter, because
                    // update_is_muted future was resolved
                    match self.snapshot.direction {
                        proto::Direction::Send => {
                            self.peer_snapshot
                                .senders_update_in_progress
                                .mutate(|mut c| *c -= 1);
                        }
                        proto::Direction::Recv => {
                            self.peer_snapshot
                                .receivers_update_in_progress
                                .mutate(|mut c| *c -= 1);
                        }
                    }
                }
            }
        });
    }

    async fn update_is_muted(&self, is_muted: bool) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        println!("Track [dir = {:?}] updated.", self.snapshot.direction);
        *self.is_muted.borrow_mut() = is_muted;
    }
}

struct Peer {
    tracks: RefCell<Vec<Track>>,
    snapshot: Rc<snapshot::Peer>,
}

impl Peer {
    fn new(snapshot: Rc<snapshot::Peer>) -> Rc<Self> {
        let this = Rc::new(Self {
            tracks: RefCell::default(),
            snapshot,
        });

        Rc::clone(&this).spawn_on_track_created();
        Rc::clone(&this).spawn_on_ice_restart();
        Rc::clone(&this).spawn_on_negotiation_role();

        this
    }

    /// Spawns [`Peer::negotiation_role`] listener.
    ///
    /// Will start negotiation process on [`Peer::negotiation_role`] update.
    fn spawn_on_negotiation_role(self: Rc<Self>) {
        let mut on_negotiation_role = Box::pin(
            self.snapshot
                .negotiation_role
                .subscribe()
                .filter_map(|q| future::ready(q)),
        );
        spawn_local(async move {
            while let Some(new_role) = on_negotiation_role.next().await {
                // wait until ice will be restarted
                self.snapshot.restart_ice.when_eq(false).await;
                match new_role {
                    snapshot::NegotiationRole::Offerer => {
                        // wait until all senders and receivers will be
                        // created/updated.
                        future::join(
                            self.snapshot.senders_update_in_progress.when_eq(0),
                            self.snapshot
                                .receivers_update_in_progress
                                .when_eq(0),
                        )
                        .await;
                        let sdp_offer = self.get_offer().await;
                        self.snapshot.sdp_offer.set(Some(sdp_offer));
                        // wait until partner Peer will provided his SDP offer
                        self.snapshot
                            .remote_sdp_offer
                            .subscribe()
                            .skip(1)
                            .filter_map(|o| future::ready(o))
                            .next()
                            .await;
                        // reset negotiation_role to None
                        self.snapshot.negotiation_finished();
                    }
                    snapshot::NegotiationRole::Answerer(remote_offer) => {
                        // wait until all receivers will be created/updated
                        self.snapshot
                            .receivers_update_in_progress
                            .when_eq(0)
                            .await;

                        self.set_remote_offer(remote_offer.clone()).await;
                        self.snapshot
                            .remote_sdp_offer
                            .set(Some(remote_offer.clone()));

                        // wait until all senders will be created/updated
                        self.snapshot
                            .senders_update_in_progress
                            .when_eq(0)
                            .await;

                        let sdp_offer = self.get_offer().await;
                        self.snapshot.sdp_offer.set(Some(sdp_offer));

                        // reset negotiation_role to None
                        self.snapshot.negotiation_finished();
                    }
                }
            }
        });
    }

    /// Spawns task which will create new [`Track`]s.
    fn spawn_track_creation_task(self: Rc<Self>, track: Rc<snapshot::Track>) {
        spawn_local(async move {
            match track.direction {
                Direction::Send => {
                    if let Some(NegotiationRole::Answerer(_)) =
                        self.snapshot.negotiation_role.borrow().clone()
                    {
                        // wait until all receivers will be created/updated
                        self.snapshot
                            .remote_sdp_offer
                            .subscribe()
                            .skip(1)
                            .filter(|s| future::ready(s.is_some()))
                            .next()
                            .await;
                    }
                }
                _ => (),
            }

            let track = Track::new(track, Rc::clone(&self.snapshot));

            // decrease scheduled changes counter
            match track.snapshot.direction {
                Direction::Send => {
                    self.snapshot
                        .senders_update_in_progress
                        .mutate(|mut c| *c -= 1);
                }
                Direction::Recv => {
                    self.snapshot
                        .receivers_update_in_progress
                        .mutate(|mut c| *c -= 1);
                }
            }
        });
    }

    fn spawn_on_track_created(self: Rc<Self>) {
        let mut on_track_created = self.snapshot.tracks.borrow().on_push();
        spawn_local(async move {
            while let Some(track) = on_track_created.next().await {
                Rc::clone(&self).spawn_track_creation_task(track);
            }
        });
    }

    /// Spawns listener for the ICE restart requests.
    fn spawn_on_ice_restart(self: Rc<Self>) {
        let mut on_ice_restart =
            Box::pin(self.snapshot.restart_ice.subscribe().filter_map(
                |is_ice_restart| async move {
                    if is_ice_restart {
                        Some(())
                    } else {
                        None
                    }
                },
            ));
        spawn_local(async move {
            while let Some(_) = on_ice_restart.next().await {
                self.restart_ice().await;
                self.snapshot.restart_ice.set(false);
            }
        });
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
                    this.peers.borrow_mut().push(Peer::new(peer));
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
