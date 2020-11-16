pub struct Track {
    pub id: u32,
    pub is_muted: bool,
    pub direction: Direction,
}

pub struct TrackPatch {
    pub id: u32,
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
