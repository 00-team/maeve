use futures::prelude::*;
use std::{sync::Arc, time::Duration};
use tokio::{signal::unix::SignalKind, sync::mpsc};
use tsclientlib::{
    ChannelId, Connection, DisconnectOptions, Identity, MessageTarget,
    OutCommandExt, StreamItem,
};

mod audio;
mod command;
mod error;
mod logger;
mod state;
mod utils;

pub use error::MaeveError;

use crate::{command::MaeveCommand, state::MaeveState};

// const PLAYLIST_END_DUR: Duration = Duration::from_secs(2);
const PACKET_DELAY: Duration = Duration::from_micros(20000);

#[tokio::main]
async fn main() -> Result<(), MaeveError> {
    log::set_logger(&logger::MasterLogger).expect("could not init logger");
    log::set_max_level(log::LevelFilter::Trace);

    let mut sigterm =
        tokio::signal::unix::signal(SignalKind::terminate()).unwrap();

    let state = Arc::new(MaeveState::new());

    let (send_audio, mut recv_audio) = mpsc::channel(1);
    let (send_text, mut recv_text) = mpsc::channel(100);

    for p in std::env::args().skip(1) {
        state.queue_add(p).await;
    }

    let adst = state.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            loop {
                let Some(name) = adst.queue_front().await else {
                    adst.queue_notified().await;
                    continue;
                };

                let Ok(packets) = audio::audio_render(&name).await else {
                    adst.queue_pop_front().await;
                    continue;
                };
                adst.add_song(state::Song { name, packets }).await;
                adst.queue_pop_front().await;
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

    let ps = state.clone();
    tokio::spawn(async move {
        let state = ps;
        'pll: loop {
            let Some(song) = state.current_song().await else {
                log::info!("no more song");
                state.current_notified().await;
                continue;
            };
            let current = state.current_index();

            let name = song.name.clone();
            log::info!("song: {name}");
            // let _ = send_text.send(format!("now playing: {name}")).await;

            let mut interval = tokio::time::interval(PACKET_DELAY);
            for p in song.packets {
                if current != state.current_index() {
                    continue 'pll;
                }

                while !state.playing() {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    // state.playing_notified().await;
                    interval.reset();
                    continue;
                }

                interval.tick().await;
                let _ = send_audio.send(p).await;
            }

            state.next();
        }
    });

    loop {
        // let t2a = audiodata.ts2a.clone();
        let events = con.events().try_for_each(|e| async {
            let StreamItem::BookEvents(ees) = e else { return Ok(()) };

            for e in ees {
                let tsclientlib::events::Event::Message {
                    target,
                    invoker,
                    message,
                } = e
                else {
                    continue;
                };

                let MessageTarget::Client(chid) = target else { continue };
                if invoker.id == chid {
                    continue;
                }

                let Some(cmd) = MaeveCommand::parse_str(&message) else {
                    continue;
                };

                match cmd {
                    MaeveCommand::Play => state.play(),
                    MaeveCommand::Pause => state.pause(),
                    MaeveCommand::Jump(x) => state.jump(x),
                    MaeveCommand::Remove(x) => state.remove_song(x).await,
                    MaeveCommand::Next => state.next(),
                    MaeveCommand::Past => state.past(),
                    MaeveCommand::Help => {
                        let _ = send_text
                            .send((
                                invoker.id,
                                MaeveCommand::help().to_string(),
                            ))
                            .await;
                    }
                    MaeveCommand::Add(name) => {
                        let list = utils::do_ls(&format!("music/{name}"));
                        if list.is_empty() {
                            const ERR: &str =
                                "[COLOR=#ff0000]NO FILE WAS FOUND[/COLOR]";
                            let _ = send_text
                                .send((
                                    invoker.id,
                                    format!("using \"{name}\" {ERR}"),
                                ))
                                .await;
                            return Ok(());
                        }

                        for p in list {
                            state.queue_add(p).await;
                        }

                        let _ = send_text
                            .send((invoker.id, state.pl_list().await))
                            .await;
                    }
                    MaeveCommand::List => {
                        let _ = send_text
                            .send((invoker.id, state.pl_list().await))
                            .await;
                    }
                    MaeveCommand::Clear => state.pl_clear().await,
                    MaeveCommand::QueueClear => state.queue_clear().await,
                }
            }

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
                let Some((chid, text)) = send_text else { continue };
                let Ok(state) = con.get_state() else { continue };
                let _ = state.send_message(MessageTarget::Client(chid), &text).send(&mut con);
            }
            _ = tokio::signal::ctrl_c() => { break; }
            _ = sigterm.recv() => { break; }
            r = events => {
                r.unwrap();
                break;
            }
        };
    }

    log::info!("yoo diss");

    // Disconnect
    con.disconnect(DisconnectOptions::new()).unwrap();
    con.events().for_each(|_| future::ready(())).await;

    Ok(())
}
