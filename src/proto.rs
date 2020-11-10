pub enum Event {
    PeerCreated { tracks: Vec<Track> },

    TrackUpdate { changes: Vec<TrackChange> },
}

pub struct Track {
    pub is_muted: bool,
    pub direction: Direction,
}

pub struct TrackPatch {
    pub is_muted: Option<bool>,
}

pub enum TrackChange {
    Added(Track),
    Update(TrackPatch),
    IceRestart,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Send,
    Recv,
}
