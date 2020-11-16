mod component;
mod proto;
mod sys;

use std::{rc::Rc, time::Duration};

use tokio::{task, task::spawn_local, time::sleep};

use crate::proto::{Direction, NegotiationRole};

#[tokio::main]
async fn main() {
    task::LocalSet::new()
        .run_until(async {
            let room = EventHandler::new();
            room.peer_created(
                vec![
                    proto::Track {
                        id: 0,
                        direction: Direction::Send,
                        is_muted: false,
                    },
                    proto::Track {
                        id: 1,
                        direction: Direction::Recv,
                        is_muted: false,
                    },
                ],
                NegotiationRole::Offerer,
            );
            spawn_local({
                let room = Rc::clone(&room);
                async move {
                    sleep(Duration::from_secs(2)).await;
                    room.sdp_answer_made("aaa".to_string());
                }
            });
            room.wait_for_negotiation_finish().await;

            print!("\n\n\n\n");

            room.tracks_applied(
                vec![
                    proto::TrackChange::IceRestart,
                    proto::TrackChange::Update(proto::TrackPatch {
                        id: 0,
                        is_muted: Some(true),
                    }),
                    proto::TrackChange::Update(proto::TrackPatch {
                        id: 1,
                        is_muted: Some(true),
                    }),
                ],
                Some(NegotiationRole::Answerer("asdkj".to_string())),
            );
            room.wait_for_negotiation_finish().await;

            print!("\n\n\n\n");

            room.tracks_applied(
                vec![proto::TrackChange::IceRestart],
                Some(NegotiationRole::Answerer("asdkj".to_string())),
            );
            room.wait_for_negotiation_finish().await;

            print!("\n\n\n\n");

            room.tracks_applied(
                vec![
                    proto::TrackChange::Added(proto::Track {
                        id: 0,
                        is_muted: false,
                        direction: Direction::Recv,
                    }),
                    proto::TrackChange::Added(proto::Track {
                        id: 1,
                        is_muted: false,
                        direction: Direction::Send,
                    }),
                ],
                Some(NegotiationRole::Answerer("aasd".to_string())),
            );
            room.wait_for_negotiation_finish().await;
            print!("\n\n\n\n");

            room.tracks_applied(
                vec![
                    proto::TrackChange::Added(proto::Track {
                        id: 0,
                        is_muted: false,
                        direction: Direction::Recv,
                    }),
                    proto::TrackChange::Added(proto::Track {
                        id: 1,
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

            room.wait_for_negotiation_finish().await;
        })
        .await;
}

struct EventHandler(Rc<component::Room>);

impl EventHandler {
    pub fn new() -> Rc<Self> {
        let room = Rc::new(component::Room::new());
        room.spawn_tasks();
        let this = Rc::new(Self(room));

        this
    }

    fn peer_created(
        &self,
        tracks: Vec<proto::Track>,
        negotiation_role: proto::NegotiationRole,
    ) {
        self.0
            .peers
            .borrow_mut()
            .push(component::Peer::new(tracks, negotiation_role));
    }

    fn tracks_applied(
        &self,
        updates: Vec<proto::TrackChange>,
        negotiation_role: Option<NegotiationRole>,
    ) {
        let peer = self.0.peers.borrow().iter().next().unwrap().clone();
        for update in updates {
            match update {
                proto::TrackChange::IceRestart => {
                    *peer.restart_ice.borrow_mut().borrow_mut() = true;
                }
                proto::TrackChange::Update(patch) => {
                    if let Some(sender) =
                        peer.senders.borrow().iter().find(|s| s.id == patch.id)
                    {
                        if let Some(is_muted) = patch.is_muted {
                            *sender.is_muted.borrow_mut().borrow_mut() =
                                is_muted;
                        }
                    } else if let Some(receiver) = peer
                        .receivers
                        .borrow()
                        .iter()
                        .find(|r| r.id == patch.id)
                    {
                        if let Some(is_muted) = patch.is_muted {
                            *receiver.is_muted.borrow_mut().borrow_mut() =
                                is_muted;
                        }
                    }
                }
                proto::TrackChange::Added(track) => match track.direction {
                    Direction::Send => {
                        peer.senders.borrow_mut().push(component::Sender::new(
                            track.id,
                            track.is_muted,
                        ));
                    }
                    Direction::Recv => {
                        peer.receivers.borrow_mut().push(
                            component::Receiver::new(track.id, track.is_muted),
                        );
                    }
                },
            }
        }
        peer.negotiation_role.set(negotiation_role);
    }

    fn sdp_answer_made(&self, sdp_answer: String) {
        let peer = self.0.peers.borrow().iter().next().unwrap().clone();
        peer.remote_sdp_offer.set(Some(sdp_answer));
    }
}

impl EventHandler {
    async fn wait_for_negotiation_finish(&self) {
        self.0
            .peers
            .borrow()
            .iter()
            .next()
            .unwrap()
            .negotiation_role
            .when(|r| r.is_none())
            .await
            .unwrap();
    }
}
