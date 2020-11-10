use super::proto;
use crate::proto::Direction;
use medea_reactive::{Observable, ObservableCell, ObservableVec};
use std::{cell::RefCell, rc::Rc};

pub struct Room {
    pub peers: RefCell<ObservableVec<Rc<Peer>>>,
}

impl Room {
    pub fn new() -> Self {
        Self {
            peers: RefCell::new(ObservableVec::new()),
        }
    }
}

pub struct Peer {
    pub tracks: RefCell<ObservableVec<Rc<Track>>>,
    pub senders_update_in_progress: ObservableCell<u32>,
    pub receivers_update_in_progress: ObservableCell<u32>,
    pub restart_ice: ObservableCell<bool>,
    pub negotiation_role: ObservableCell<Option<NegotiationRole>>,
    pub remote_sdp_offer: ObservableCell<Option<String>>,
    pub sdp_offer: ObservableCell<Option<String>>,
}

impl Peer {
    pub fn new(
        tracks: Vec<proto::Track>,
        negotiation_role: NegotiationRole,
    ) -> Rc<Self> {
        let mut receivers_update_in_progress = 0;
        let mut senders_update_in_progress = 0;
        let tracks: Vec<_> = tracks
            .into_iter()
            .map(|t| {
                match t.direction {
                    Direction::Send => senders_update_in_progress += 1,
                    Direction::Recv => receivers_update_in_progress += 1,
                }

                Track::new(t)
            })
            .collect();
        Rc::new(Self {
            tracks: RefCell::new(tracks.into()),
            negotiation_role: ObservableCell::new(Some(negotiation_role)),
            restart_ice: ObservableCell::new(false),
            receivers_update_in_progress: ObservableCell::new(
                receivers_update_in_progress,
            ),
            senders_update_in_progress: ObservableCell::new(
                senders_update_in_progress,
            ),
            remote_sdp_offer: ObservableCell::new(None),
            sdp_offer: ObservableCell::new(None),
        })
    }

    pub fn negotiation_finished(&self) {
        self.negotiation_role.set(None);
    }
}

pub struct Track {
    pub is_muted: ObservableCell<bool>,
    pub direction: Direction,
}

impl Track {
    pub fn new(track: proto::Track) -> Rc<Self> {
        Rc::new(Self {
            is_muted: ObservableCell::new(track.is_muted),
            direction: track.direction,
        })
    }

    pub fn transaction(self: Rc<Self>, peer: Rc<Peer>) -> TrackTransaction {
        TrackTransaction { track: self, peer }
    }
}

pub struct TrackTransaction {
    track: Rc<Track>,
    peer: Rc<Peer>,
}

impl Drop for TrackTransaction {
    fn drop(&mut self) {
        let mutation_count = 1;
        match self.track.direction {
            Direction::Send => {
                self.peer
                    .senders_update_in_progress
                    .mutate(|mut p| *p += mutation_count);
            }
            Direction::Recv => {
                self.peer
                    .receivers_update_in_progress
                    .mutate(|mut p| *p += mutation_count);
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NegotiationRole {
    Offerer,
    Answerer(String),
}
