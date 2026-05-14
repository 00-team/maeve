use futures::prelude::*;
use std::io::Cursor;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::task::LocalSet;
use tokio::{io::AsyncWriteExt, sync::mpsc};

mod logger;

use tsclientlib::{
    ChannelId, ClientId, Connection, DisconnectOptions, FiletransferHandle, Identity, StreamItem,
};
use tsproto_packets::packets::{AudioData, CodecType, OutAudio};
// use tsproto_packets::packets::AudioData;

// mod audio_utils;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ConnectionId(u64);

#[derive(Debug)]
struct MaeveError {}

#[tokio::main]
async fn main() -> Result<(), MaeveError> {
    log::set_logger(&logger::MasterLogger).expect("could not init logger");
    log::set_max_level(log::LevelFilter::Trace);

    let path = std::env::args().nth(1).expect("no audio");

    let ffmpeg = Command::new("ffmpeg")
        .args(&[
            // "-loglevel",
            // "quiet",
            "-i",
            &path,
            "-af",
            "aresample=48000",
            "-f",
            "s16be",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("no ffmpeg");

    // const LAMS: &[u8] = include_bytes!("../audio/jf.opus");
    // let li16: &[i16] = bytemuck::cast_slice(LAMS);

    // Assuming you have a Vec<u8> raw_data from the file

    // let (lams_data, _) = ogg_opus::decode::<_, 48000>(Cursor::new(LAMS)).expect("invalid opus");

    // let (lams_info, lams_data) = shravan::codec::open(LAMS).unwrap();

    let con_id = ConnectionId(1);
    let local_set = LocalSet::new();
    // let audiodata = audio_utils::start(&local_set)?;

    // let con_config = Connection::build("185.209.42.64")
    let con_config = Connection::build("127.0.0.1")
        .log_commands(false)
        .channel_id(ChannelId(1))
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

    let r = con
        .events()
        .try_filter(|e| future::ready(matches!(e, StreamItem::BookEvents(_))))
        .next()
        .await;
    if let Some(r) = r {
        r.unwrap();
    }

    // const AVATAR: &[u8] = include_bytes!("../avatar/avatar2.png");
    //
    // let fth = con
    //     .upload_file(
    //         ChannelId(0),
    //         "/avatar",
    //         None,
    //         AVATAR.len() as u64,
    //         true,
    //         false,
    //     )
    //     .unwrap();

    let (send, mut recv) = mpsc::channel(5);

    let mut ffout = ffmpeg.stdout.unwrap();
    let mut audio_out = Vec::with_capacity(50 * 1024 * 1024);
    ffout.read_to_end(&mut audio_out).await.unwrap();
    let samples: Vec<i16> = audio_out
        .chunks_exact(2)
        .map(|chunk| i16::from_be_bytes([chunk[0], chunk[1]]))
        .collect();

    let mut err = String::with_capacity(50 * 1024);
    ffmpeg
        .stderr
        .unwrap()
        .read_to_string(&mut err)
        .await
        .unwrap();
    log::info!("stderr: {err}");

    let encoder = audiopus::coder::Encoder::new(
        audiopus::SampleRate::Hz48000,
        audiopus::Channels::Stereo,
        audiopus::Application::Audio,
    )
    .expect("Could not create encoder");

    tokio::spawn(async move {
        let mut id = 0;

        const FRAME_SIZE: usize = 960;
        const MAX_PACKET_SIZE: usize = 3 * 1276;

        let mut pcm_in_be: [i16; FRAME_SIZE * 2] = [0; FRAME_SIZE * 2];
        let mut opus_pkt: [u8; MAX_PACKET_SIZE] = [0; MAX_PACKET_SIZE];

        for chunk in samples.chunks(FRAME_SIZE * 2) {
            for (i, d) in chunk.iter().enumerate() {
                pcm_in_be[i] = (*d as f32 * 0.5) as i16;
            }
            let len = encoder.encode(&pcm_in_be, &mut opus_pkt[..]).unwrap();

            let packet = OutAudio::new(&AudioData::C2S {
                id,
                codec: CodecType::OpusMusic,
                data: &opus_pkt[..len],
            });
            id += 1;

            send.send(packet).await.unwrap();

            tokio::time::sleep(Duration::from_micros(17000)).await;
        }

        // for chunk in LAMS.chunks((48000.0 * 0.002) as usize * 2) {
        //     // log::info!("data[i16]: {:?}", &chunk[0..10]);
        //     let data = bytemuck::cast_slice(chunk);
        //     // log::info!("data[u8]: {:?}", &data[0..20]);
        //     let packet = OutAudio::new(&AudioData::C2S {
        //         id: 0,
        //         codec: CodecType::OpusVoice,
        //         data,
        //     });
        //     // id += 1;
        //     send.send(packet).await.unwrap();
        // }

        // loop {
        //     tokio::time::sleep(Duration::from_millis(100)).await;
        // }
    });

    // {
    //     let mut a2t = audiodata.a2ts.lock().unwrap();
    //     a2t.set_listener(send);
    //     a2t.set_volume(args.volume);
    //     a2t.set_playing(true);
    // }

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

        // Wait for ctrl + c
        tokio::select! {
            send_audio = recv.recv() => {
                if let Some(packet) = send_audio {
                    con.send_audio(packet).unwrap();
                } else {
                    log::info!("Audio sending stream was canceled");
                    break;
                }
            }
            _ = tokio::signal::ctrl_c() => { break; }
            r = events => {
                r.unwrap();
                break;
                // bail!("Disconnected");
            }
        };
    }

    log::info!("yoo diss");

    // Disconnect
    con.disconnect(DisconnectOptions::new()).unwrap();
    con.events().for_each(|_| future::ready(())).await;

    Ok(())
}
