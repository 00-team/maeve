use futures::prelude::*;
use std::{sync::Arc, time::Duration};
use tokio::sync::{RwLock, mpsc};
use tsclientlib::{
    ChannelId, Connection, DisconnectOptions, Identity, MessageTarget,
    OutCommandExt, StreamItem,
};
use tsproto_packets::packets::OutPacket;

mod audio;
mod error;
mod logger;

pub use error::MaeveError;

#[derive(Clone)]
struct Song {
    packets: Vec<OutPacket>,
    name: String,
}

const PLAYLIST_END_DUR: Duration = Duration::from_secs(2);
const PACKET_DELAY: Duration = Duration::from_micros(20000);

#[tokio::main]
async fn main() -> Result<(), MaeveError> {
    log::set_logger(&logger::MasterLogger).expect("could not init logger");
    log::set_max_level(log::LevelFilter::Trace);

    let playlist = Arc::new(RwLock::new(Vec::<Song>::with_capacity(256)));
    let current_playing = RwLock::new(0usize);

    let args = std::env::args().skip(1).collect::<Vec<_>>();

    let (send_audio, mut recv_audio) = mpsc::channel(1);
    let (send_text, mut recv_text) = mpsc::channel(100);

    let adpp = playlist.clone();
    let adsx = send_text.clone();
    let t = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            for p in args {
                let Ok(packets) = audio::audio_render(&p).await else {
                    continue;
                };
                adpp.write().await.push(Song { name: p.clone(), packets });
                let _ = adsx.send(format!("added {p} to playlist")).await;
            }
        });
    });

    // let con_config = Connection::build("185.209.42.64")
    let con_config = Connection::build("127.0.0.1")
        .log_commands(false)
        .channel_id(ChannelId(1))
        // .channel("Underground")
        .name("Maeve");

    // Optionally set the key of this client, otherwise a new key is generated.
    let id = Identity::new_from_str(
        "MG0DAgeAAgEgAiAIXJBlj1hQbaH0Eq0DuLlCmH8bl+veTAO2+\
		k9EQjEYSgIgNnImcmKo7ls5mExb6skfK2Tw+u54aeDr0OP1ITs\
		C/50CIA8M5nmDBnmDM/gZ//4AAAAAAAAAAAAAAAAAAAAZRzOI",
    )
    .unwrap();
    let con_config = con_config.identity(id);

    // Connect
    let mut con = con_config.connect().unwrap();

    // wait to connect
    let r = con
        .events()
        .try_filter(|e| future::ready(matches!(e, StreamItem::BookEvents(_))))
        .next()
        .await;
    if let Some(r) = r {
        r.unwrap();
    }

    tokio::spawn(async move {
        loop {
            let song = {
                let px = *current_playing.read().await;
                let pp = playlist.read().await;
                if px >= pp.len() {
                    log::info!("no more song");
                    tokio::time::sleep(PLAYLIST_END_DUR).await;
                    continue;
                }

                pp[px].clone()
            };

            let name = song.name.clone();
            log::info!("song: {name}");
            let _ = send_text.send(format!("now playing: {name}")).await;

            let mut interval = tokio::time::interval(PACKET_DELAY);
            for p in song.packets {
                interval.tick().await;
                let _ = send_audio.send(p).await;
            }

            *current_playing.write().await += 1;
        }
    });

    loop {
        // let t2a = audiodata.ts2a.clone();
        let events = con.events().try_for_each(|e| async {
            match e {
                // StreamItem::FileUpload(a, mut b) => {
                //     log::info!("uploading");
                //     assert_eq!(a, fth);
                //     b.stream.write_all(AVATAR).await.unwrap();
                //     log::info!("uploaded: {}", AVATAR.len());
                // }
                // StreamItem::FiletransferFailed(a, b) => {
                //     log::error!("ftf: {b:#?}");
                // }
                _ => {}
            }

            // if let StreamItem::Audio(packet) = e {
            //     // let from = ClientId(match packet.data().data() {
            //     //     AudioData::S2C { from, .. } => *from,
            //     //     AudioData::S2CWhisper { from, .. } => *from,
            //     //     _ => panic!("Can only handle S2C packets but got a C2S packet"),
            //     // });
            //     // let mut t2a = t2a.lock().unwrap();
            //     // if let Err(error) = t2a.play_packet((con_id, from), packet) {
            //     //     debug!(%error, "Failed to play packet");
            //     // }
            // }
            Ok(())
        });

        tokio::select! {
            send_audio = recv_audio.recv() => {
                if let Some(packet) = send_audio {
                    con.send_audio(packet).unwrap();
                } else {
                    log::info!("Audio sending stream was canceled");
                    break;
                }
            }
            send_text = recv_text.recv() => {
                let Some(text) = send_text else { continue };
                let Ok(state) = con.get_state() else { continue };
                let _ = state.send_message(MessageTarget::Channel, &text).send(&mut con);
            }
            _ = tokio::signal::ctrl_c() => { break; }
            r = events => {
                r.unwrap();
                break;
            }
        };
    }

    let _ = t.join();
    log::info!("yoo diss");

    // Disconnect
    con.disconnect(DisconnectOptions::new()).unwrap();
    con.events().for_each(|_| future::ready(())).await;

    Ok(())
}
