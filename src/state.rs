use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::sync::{Notify, RwLock};
use tsproto_packets::packets::OutPacket;

#[derive(Clone)]
pub struct Song {
    pub packets: Vec<OutPacket>,
    pub name: String,
}

pub struct MaeveState {
    playing: AtomicBool,
    playing_notify: Notify,
    // current_playing: RwLock<usize>,
    current_playing: AtomicUsize,
    currnet_notify: Notify,
    playlist: RwLock<Vec<Song>>,
    queued: RwLock<VecDeque<String>>,
    queue_notify: Notify,
}

impl MaeveState {
    pub fn new() -> Self {
        Self {
            playing: AtomicBool::new(true),
            playing_notify: Notify::new(),
            current_playing: AtomicUsize::new(0),
            currnet_notify: Notify::new(),
            queue_notify: Notify::new(),
            playlist: Default::default(),
            queued: Default::default(),
        }
    }

    pub fn pause(&self) {
        self.playing.store(false, Ordering::Relaxed);
    }

    pub fn play(&self) {
        self.playing.store(true, Ordering::Relaxed);
        self.playing_notify.notify_one();
    }

    pub async fn add_song(&self, song: Song) {
        self.playlist.write().await.push(song);
        self.currnet_notify.notify_one();
    }

    pub async fn remove_song(&self, index: usize) {
        let mut pl = self.playlist.write().await;
        if pl.len() <= index {
            return;
        }

        pl.remove(index);
        self.currnet_notify.notify_one();
    }

    pub fn jump(&self, index: usize) {
        self.current_playing.store(index, Ordering::Relaxed);
        self.currnet_notify.notify_one();
    }

    pub fn next(&self) {
        self.current_playing.fetch_add(1, Ordering::Relaxed);
        self.currnet_notify.notify_one();
    }

    pub fn past(&self) {
        self.current_playing.fetch_sub(1, Ordering::Relaxed);
        self.currnet_notify.notify_one();
    }

    pub async fn current_song(&self) -> Option<Song> {
        let pl = self.playlist.read().await;
        let cx = self.current_playing.load(Ordering::Relaxed);
        pl.get(cx).cloned()
    }

    pub fn current_index(&self) -> usize {
        self.current_playing.load(Ordering::Relaxed)
    }

    pub async fn current_notified(&self) {
        self.currnet_notify.notified().await;
    }

    pub async fn playing_notified(&self) {
        self.currnet_notify.notified().await;
    }

    pub async fn queue_notified(&self) {
        self.queue_notify.notified().await;
    }

    pub fn playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    pub async fn pl_list(&self) -> String {
        let mut out = String::with_capacity(1024);
        out.push_str("\ncurrent play list:\n\n");

        let cx = self.current_index();
        let pl = self.playlist.read().await;
        let pl_len = pl.len();
        for (i, s) in pl.iter().enumerate() {
            let name = &s.name;
            if i == cx {
                out += &format!("> [COLOR=#0fff0f]{i}[/COLOR] [B]{name}[/B]\n");
                continue;
            }

            out += &format!("[COLOR=#00ffff]{i}[/COLOR] {name}\n");
        }


        let q = self.queued.read().await;
        if q.is_empty() {
            return out;
        }

        out.push_str("\nqueued:\n");
        for (i, p) in q.iter().enumerate() {
            out += &format!("[COLOR=#f5deb3]{}[/COLOR] {p}\n", i + pl_len);
        }

        out
    }

    pub async fn queue_front(&self) -> Option<String> {
        self.queued.read().await.front().cloned()
    }

    pub async fn queue_pop_front(&self) {
        let _ = self.queued.write().await.pop_front();
    }

    pub async fn queue_add(&self, path: String) {
        self.queued.write().await.push_back(path);
        self.queue_notify.notify_one();
    }
}
