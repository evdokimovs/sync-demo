use std::{cell::RefCell, rc::Rc};

use futures::StreamExt as _;
use medea_reactive::ProgressableObservable;
use tokio::task::spawn_local;

use crate::sys::MediaStreamTrack;

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
