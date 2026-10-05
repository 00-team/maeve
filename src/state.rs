use rand::seq::SliceRandom;
use std::collections::HashSet;
use std::ops::Range;
use std::{
    collections::VecDeque,
    hash::{DefaultHasher, Hash, Hasher},
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};
use tokio::sync::{Notify, RwLock};
use tsproto_packets::packets::OutPacket;

use crate::PACKET_DELAY;
use crate::utils::fmt_dur;

#[derive(Clone)]
pub struct Song {
    pub packets: Vec<OutPacket>,
    pub name: String,
    pub hash: u64,
}

impl Song {
    pub const fn duration(&self) -> Duration {
        Duration::from_millis(
            self.packets.len() as u64 * PACKET_DELAY.as_millis() as u64,
        )
    }

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
    current_packet: AtomicUsize,
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
            current_packet: AtomicUsize::new(0),
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

    pub fn current_packet(&self) -> usize {
        self.current_packet.load(Ordering::Relaxed)
    }
    pub fn current_packet_and_next(&self) -> usize {
        self.current_packet.fetch_add(1, Ordering::Relaxed)
    }
    pub fn set_current_packet(&self, cp: usize) {
        self.current_packet.store(cp, Ordering::Relaxed);
    }
    pub fn current_duration(&self) -> Duration {
        Duration::from_millis(
            self.current_packet() as u64 * PACKET_DELAY.as_millis() as u64,
        )
    }
    pub fn seek(&self, secs: u64) {
        let idx = secs as usize * 1000 / PACKET_DELAY.as_millis() as usize;
        self.set_current_packet(idx);
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

    pub async fn remove_range(&self, range: Range<usize>) -> Range<usize> {
        let mut pl = self.playlist.write().await;
        let range = range.start..pl.len().min(range.end);
        pl.drain(range.clone());
        let cdx = self.current_index();
        if range.contains(&cdx) {
            self.current_playing.store(range.start, Ordering::SeqCst);
        } else if cdx >= range.end {
            self.current_playing.fetch_sub(range.len(), Ordering::SeqCst);
        }
        self.update_hash().await;
        self.current_notify.notify_one();

        range
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
        self.set_current_packet(0);
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
        let song = pl.get(cx).cloned()?;

        if song.hash != self.current_hash() {
            self.current_hash.store(song.hash, Ordering::Relaxed);
        }

        Some(song)
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

    pub async fn pl_list(&self, range: Option<Range<usize>>) -> String {
        let mut out = String::with_capacity(1024);

        let cx = self.current_index();
        let pl = self.playlist.read().await;
        let cx = cx.min(pl.len().saturating_sub(1));

        let range = if let Some(range) = range {
            if range.end == 999 && range.start == 0 {
                0..pl.len()
            } else {
                let end = range.end.min(pl.len());
                let start = range.start.min(end.saturating_sub(1));
                start..end
            }
        } else {
            let s = cx.saturating_sub(4);
            let end = (s + 10).min(pl.len());
            let s = if end - s < 10 { end.saturating_sub(10) } else { s };
            s..end
        };

        out += &format!("\ncurrent playlist: {range:?}\n");

        if self.loop_playlist() {
            out.push_str("> looping playlist");
        } else if self.loop_song() {
            out.push_str("> looping current song");
        }

        out.push_str("\n\n");

        let offset = range.start;
        for (i, s) in pl[range].iter().enumerate() {
            let name = &s.name;
            let ci = offset + i;
            let tt_dur = fmt_dur(s.duration());

            if ci == cx {
                let pp_dur = fmt_dur(self.current_duration());
                out += &format!(
                    "{} [COLOR=#0fff0f]{ci}[/COLOR] [B]{name}[/B] {pp_dur}/{tt_dur}\n",
                    if self.playing() { ">" } else { "|" }
                );
                continue;
            }

            out += &format!("[COLOR=#00ffff]{ci}[/COLOR] {name} | {tt_dur}\n");
        }

        out
    }

    pub async fn find(&self, name: String) -> String {
        let mut out = String::with_capacity(1024);

        let cx = self.current_index();
        let pl = self.playlist.read().await;

        out += &format!("\nfinding [B]\"{name}\"[/B] in playlist\n\n");
        let name_bold = format!("[B]{name}[/B]");

        for (i, s) in pl.iter().enumerate() {
            if !s.name.contains(&name) {
                continue;
            }
            let sname = s.name.replace(&name, &name_bold);
            let tt_dur = fmt_dur(s.duration());

            if i == cx {
                let pp_dur = fmt_dur(self.current_duration());
                out += &format!(
                    "{} [COLOR=#0fff0f]{i}[/COLOR] {sname} {pp_dur}/{tt_dur}\n",
                    if self.playing() { ">" } else { "|" }
                );
                continue;
            }

            out += &format!("[COLOR=#00ffff]{i}[/COLOR] {sname} | {tt_dur}\n");
        }

        out
    }

    pub async fn info(&self) -> String {
        let mut out = String::with_capacity(1024);
        out += "\ninfo:\n";

        if self.loop_playlist() {
            out.push_str("> looping playlist");
        } else if self.loop_song() {
            out.push_str("> looping current song");
        }

        let pl_len = self.playlist.read().await.len();
        let q_len = self.queued.read().await.len();
        out += &format!("playlist: {pl_len}\nqueue: {q_len}\n");

        if let Some(cs) = self.current_song().await {
            let ci = self.current_index();
            let pp_dur = fmt_dur(self.current_duration());
            let tt_dur = fmt_dur(cs.duration());

            out += &format!(
                "{} [COLOR=#0fff0f]{ci}[/COLOR] [B]{}[/B] {pp_dur}/{tt_dur}\n",
                if self.playing() { ">" } else { "|" },
                cs.name,
            );
        }

        out
    }

    pub async fn q_list(&self) -> String {
        let mut out = String::with_capacity(1024);
        out += "\nqueue:\n\n";

        let pl_len = self.playlist.read().await.len();
        let q = self.queued.read().await;
        if q.is_empty() {
            return out;
        }

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

    pub async fn queue_add(&self, path: Vec<String>) {
        self.queued.write().await.extend(path);
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

    pub async fn dedup(&self) {
        let mut saw = HashSet::with_capacity(1024);

        fn fname(path: &str) -> String {
            path.rsplit('/')
                .next()
                .map(|v| v.to_lowercase())
                .unwrap_or_default()
        }

        self.playlist.write().await.retain(|s| saw.insert(fname(&s.name)));
        self.queued.write().await.retain(|s| saw.insert(fname(s)));
    }
}
