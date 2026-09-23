use rand::seq::SliceRandom;
use std::{
    collections::VecDeque,
    hash::{DefaultHasher, Hash, Hasher},
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use tokio::sync::{Notify, RwLock};
use tsproto_packets::packets::OutPacket;

#[derive(Clone)]
pub struct Song {
    pub packets: Vec<OutPacket>,
    pub name: String,
    pub hash: u64,
}

impl Song {
    pub fn hash(name: &str, size: usize) -> u64 {
        let mut hashar = DefaultHasher::default();
        name.hash(&mut hashar);
        size.hash(&mut hashar);
        hashar.finish()
    }
}

pub struct MaeveState {
    playing: AtomicBool,
    playing_notify: Notify,
    current_playing: AtomicUsize,
    current_hash: AtomicU64,
    current_notify: Notify,
    playlist: RwLock<Vec<Song>>,
    queued: RwLock<VecDeque<String>>,
    queue_notify: Notify,
    loop_playlist: AtomicBool,
    loop_song: AtomicBool,
}

impl MaeveState {
    pub fn new() -> Self {
        Self {
            playing: AtomicBool::new(true),
            playing_notify: Notify::new(),
            current_playing: AtomicUsize::new(0),
            current_hash: AtomicU64::new(0),
            current_notify: Notify::new(),
            queue_notify: Notify::new(),
            playlist: Default::default(),
            queued: Default::default(),
            loop_song: AtomicBool::new(false),
            loop_playlist: AtomicBool::new(false),
        }
    }

    pub fn pause(&self) {
        self.playing.store(false, Ordering::Relaxed);
    }

    pub fn play(&self) {
        if !self.playing.fetch_xor(true, Ordering::SeqCst) {
            self.playing_notify.notify_one();
        }
    }

    pub fn loop_song(&self) -> bool {
        self.loop_song.load(Ordering::Relaxed)
    }

    pub fn loop_playlist(&self) -> bool {
        self.loop_playlist.load(Ordering::Relaxed)
    }

    pub fn loop_cycle(&self) {
        if self.loop_song() {
            self.loop_song.store(false, Ordering::Relaxed);
            self.loop_playlist.store(false, Ordering::Relaxed);
        } else if self.loop_playlist() {
            self.loop_song.store(true, Ordering::Relaxed);
            self.loop_playlist.store(false, Ordering::Relaxed);
        } else {
            self.loop_song.store(false, Ordering::Relaxed);
            self.loop_playlist.store(true, Ordering::Relaxed);
        }
    }

    pub async fn add_song(&self, song: Song) {
        self.playlist.write().await.push(song);
        self.current_notify.notify_one();
    }

    pub async fn remove_range(&self, range: std::ops::Range<usize>) {
        let mut pl = self.playlist.write().await;
        let range = range.start..pl.len().min(range.end);
        pl.drain(range.clone());
        let cdx = self.current_index();
        if range.contains(&cdx) {
            self.current_playing.store(range.start, Ordering::Relaxed);
        } else if cdx > range.end {
            self.current_playing.fetch_sub(range.len(), Ordering::Relaxed);
        }
        self.update_hash().await;
        self.current_notify.notify_one();
    }

    pub async fn shuffle(&self) {
        let mut pl = self.playlist.write().await;
        pl.shuffle(&mut rand::rng());
    }

    pub async fn sort(&self) {
        let mut pl = self.playlist.write().await;
        pl.sort_by_key(|s| s.name.clone());
    }

    pub async fn jump(&self, index: usize) {
        self.current_playing.store(index, Ordering::Relaxed);
        self.current_notify.notify_one();
        self.update_hash().await;
    }

    pub async fn next(&self) {
        self.current_playing.fetch_add(1, Ordering::Relaxed);
        self.current_notify.notify_one();
        self.update_hash().await;
    }

    pub async fn past(&self) {
        self.current_playing.fetch_sub(1, Ordering::Relaxed);
        self.current_notify.notify_one();
        self.update_hash().await;
    }

    pub async fn current_song(&self) -> Option<Song> {
        let pl = self.playlist.read().await;
        let cx = self.current_playing.load(Ordering::Relaxed);
        pl.get(cx).cloned()
    }

    async fn update_hash(&self) {
        let hash = if let Some(song) = self.current_song().await {
            song.hash
        } else {
            0
        };

        self.current_hash.store(hash, Ordering::Relaxed);
    }

    pub fn current_index(&self) -> usize {
        self.current_playing.load(Ordering::Relaxed)
    }

    pub fn current_hash(&self) -> u64 {
        self.current_hash.load(Ordering::Relaxed)
    }

    pub async fn current_notified(&self) {
        self.current_notify.notified().await;
    }

    pub async fn queue_notified(&self) {
        self.queue_notify.notified().await;
    }

    pub fn playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    pub async fn pl_list(&self) -> String {
        let mut out = String::with_capacity(1024);
        out.push_str("\ncurrent playlist:\n");

        if self.loop_playlist() {
            out.push_str("> looping playlist");
        } else if self.loop_song() {
            out.push_str("> looping current song");
        }

        out.push_str("\n\n");

        let cx = self.current_index();
        let pl_len = {
            let pl = self.playlist.read().await;
            let pl_len = pl.len();
            for (i, s) in pl.iter().enumerate() {
                let name = &s.name;
                if i == cx {
                    out += &format!(
                        "{} [COLOR=#0fff0f]{i}[/COLOR] [B]{name}[/B]\n",
                        if self.playing() { ">" } else { "|" }
                    );
                    continue;
                }

                out += &format!("[COLOR=#00ffff]{i}[/COLOR] {name}\n");
            }
            pl_len
        };

        let q = self.queued.read().await;
        if q.is_empty() {
            return out;
        }

        out.push_str("\nqueued:\n");
        for (i, p) in q.iter().enumerate() {
            out += &format!("[COLOR=#FF69B4]{}[/COLOR] {p}\n", i + pl_len);
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

    pub async fn queue_clear(&self) {
        self.queued.write().await.clear();
    }

    pub async fn pl_clear(&self) {
        self.playlist.write().await.clear();
        self.current_playing.store(0, Ordering::Relaxed);
        self.update_hash().await;
        self.current_notify.notify_one();
    }
}
