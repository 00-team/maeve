use std::process::Stdio;
use tokio::{io::AsyncReadExt, process::Command};
use tsproto_packets::packets::{AudioData, CodecType, OutAudio, OutPacket};

use crate::MaeveError;

pub async fn audio_render(path: &str) -> Result<Vec<OutPacket>, MaeveError> {
    let ffmpeg = Command::new("taskset")
        .args(&[
            "-c",
            "0,1",
            "ffmpeg",
            "-loglevel",
            "quiet",
            "-i",
            &path,
            "-af",
            // "loudnorm=I=-16:LRA=11:TP=-1.5",
            "loudnorm=I=-16:LRA=11:TP=-1.5,acompressor=threshold=-18dB:ratio=3:attack=20:release=250,alimiter=limit=0.95",
            "-ar",
            "48000",
            "-ac",
            "2",
            "-f",
            "s16be",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("no ffmpeg");

    let mut ffout = ffmpeg.stdout.unwrap();
    let mut audio_out = Vec::with_capacity(50 * 1024 * 1024);
    ffout.read_to_end(&mut audio_out).await.unwrap();
    let samples: Vec<i16> = audio_out
        .chunks_exact(2)
        .map(|chunk| i16::from_be_bytes([chunk[0], chunk[1]]))
        .collect();

    let mut err = String::with_capacity(50 * 1024);
    ffmpeg.stderr.unwrap().read_to_string(&mut err).await.unwrap();
    log::info!("stderr: {err}");

    let encoder = audiopus::coder::Encoder::new(
        audiopus::SampleRate::Hz48000,
        audiopus::Channels::Stereo,
        audiopus::Application::Audio,
    )
    .expect("Could not create encoder");

    let mut id = 0;

    const FRAME_SIZE: usize = 960;
    const MAX_PACKET_SIZE: usize = 3 * 1276;

    let mut pcm_in_be: [i16; FRAME_SIZE * 2] = [0; FRAME_SIZE * 2];
    let mut opus_pkt: [u8; MAX_PACKET_SIZE] = [0; MAX_PACKET_SIZE];
    let mut all_packets = Vec::with_capacity(50 * 60 * 30);
    let total_chunks = samples.len() / (FRAME_SIZE * 2);

    for (cx, chunk) in samples.chunks(FRAME_SIZE * 2).enumerate() {
        // let clen = chunk.len();
        for (i, d) in chunk.iter().enumerate() {
            // pcm_in_be[i] = (*d as f32 * 0.5) as i16;
            pcm_in_be[i] = *d;
        }
        let len = encoder.encode(&pcm_in_be, &mut opus_pkt).unwrap();

        let packet = OutAudio::new(&AudioData::C2S {
            id,
            codec: CodecType::OpusMusic,
            data: &opus_pkt[..len],
        });
        id += 1;
        all_packets.push(packet);
        if cx.is_multiple_of(1000) {
            log::info!("encoded: {cx}/{total_chunks}");
        }
    }

    Ok(all_packets)
}
