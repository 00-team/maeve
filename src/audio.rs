use std::{
    io::{Read, Write},
    path::Path,
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};
use tsproto_packets::packets::{AudioData, CodecType, OutAudio, OutPacket};

use crate::{MaeveError, PACKET_DELAY, state::Song};

pub struct MaeveAudio {
    chunks: Vec<u32>,
    opus: Vec<u8>,
}

impl MaeveAudio {
    pub fn to_packets(self) -> Vec<OutPacket> {
        let mut offset = 0usize;
        let mut all_packets = Vec::with_capacity(self.chunks.len());
        for (id, len) in self.chunks.iter().enumerate() {
            let len = *len as usize;
            let packet = OutAudio::new(&AudioData::C2S {
                id: id as u16,
                codec: CodecType::OpusMusic,
                data: &self.opus[offset..offset + len],
            });
            offset += len;
            all_packets.push(packet);
        }

        all_packets
    }

    pub fn save(&self, path: &Path) -> Result<(), MaeveError> {
        if path.is_dir() {
            return Err(MaeveError::AudioSaveFailed);
        }
        let Some(parent) = path.parent() else {
            return Err(MaeveError::AudioSaveFailed);
        };

        let _ = std::fs::create_dir_all(parent);

        fn _save(ma: &MaeveAudio, path: &Path) -> std::io::Result<()> {
            let mut file = std::fs::File::options()
                .write(true)
                .truncate(true)
                .create(true)
                .open(path)?;

            let chunks: &[u8] = bytemuck::cast_slice(&ma.chunks);
            file.write_all(&(chunks.len() as u64).to_le_bytes())?;
            file.write_all(&(ma.opus.len() as u64).to_le_bytes())?;
            file.write_all(chunks)?;
            file.write_all(&ma.opus)?;

            Ok(())
        }

        if _save(&self, path).is_err() {
            return Err(MaeveError::AudioSaveFailed);
        }

        Ok(())
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        let mut file = std::fs::File::options().read(true).open(path)?;

        // let chunks: &[u8] = bytemuck::cast_slice(&self.chunks);
        let mut chunks_len = [0u8; 8];
        file.read_exact(&mut chunks_len)?;
        let chunks_len = u64::from_le_bytes(chunks_len) as usize;

        let mut opus_len = [0u8; 8];
        file.read_exact(&mut opus_len)?;
        let opus_len = u64::from_le_bytes(opus_len) as usize;

        let mut chunks = vec![0; chunks_len];
        file.read_exact(&mut chunks)?;

        let mut opus = vec![0; opus_len];
        file.read_exact(&mut opus)?;

        Ok(Self { opus, chunks: bytemuck::cast_slice(&chunks).to_vec() })
    }
}

pub struct MaeveAudioHead {
    pub duration: Duration,
    pub size: usize,
}

impl MaeveAudioHead {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let mut file = std::fs::File::options().read(true).open(path)?;

        // let chunks: &[u8] = bytemuck::cast_slice(&self.chunks);
        let mut chunks_len = [0u8; 8];
        file.read_exact(&mut chunks_len)?;
        let chunks_len = u64::from_le_bytes(chunks_len) as usize;

        let mut opus_len = [0u8; 8];
        file.read_exact(&mut opus_len)?;
        let opus_len = u64::from_le_bytes(opus_len) as usize;

        let duration = Duration::from_millis(
            (chunks_len / 4) as u64 * PACKET_DELAY.as_millis() as u64,
        );

        Ok(Self { duration, size: opus_len })
    }
}

pub async fn audio_render(path: &str) -> Result<Song, MaeveError> {
    let cache_dir = Path::new("maeve_cache");
    let ffmpeg = Command::new("taskset")
        .args(&[
            "-c",
            "0,1,2",
            "ffmpeg",
            "-hide_banner",
            "-loglevel",
            "error",
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
    if !err.is_empty() {
        log::info!("ffmpeg err:\n{err}");
    }

    if samples.is_empty() {
        return Err(MaeveError::AudioEncodeFailed);
    }

    let encoder = audiopus::coder::Encoder::new(
        audiopus::SampleRate::Hz48000,
        audiopus::Channels::Stereo,
        audiopus::Application::Audio,
    )
    .expect("Could not create encoder");

    const FRAME_SIZE: usize = 960;
    const MAX_PACKET_SIZE: usize = 3 * 1276;

    let mut pcm_in_be: [i16; FRAME_SIZE * 2] = [0; FRAME_SIZE * 2];
    let mut opus_pkt: [u8; MAX_PACKET_SIZE] = [0; MAX_PACKET_SIZE];
    let mut audio = MaeveAudio {
        chunks: Vec::with_capacity(samples.len() / FRAME_SIZE * 2),
        opus: Vec::with_capacity(samples.len()),
    };

    for chunk in samples.chunks(FRAME_SIZE * 2) {
        let clen = chunk.len();
        pcm_in_be[..clen].copy_from_slice(chunk);
        // let clen = chunk.len();
        // for (i, d) in chunk.iter().enumerate() {
        //     // pcm_in_be[i] = (*d as f32 * 0.5) as i16;
        //     pcm_in_be[i] = *d;
        // }
        let len = encoder.encode(&pcm_in_be[..clen], &mut opus_pkt).unwrap();

        audio.chunks.push(len as u32);
        audio.opus.extend_from_slice(&opus_pkt[..len]);
        // let packet = OutAudio::new(&AudioData::C2S {
        //     id,
        //     codec: CodecType::OpusMusic,
        //     data: &opus_pkt[..len],
        // });
        // id += 1;
        // all_packets.push(packet);
    }

    let cf = cache_dir.join(path);
    audio.save(&cf)?;
    let size = audio.opus.len();

    let name = path
        .strip_prefix("music/")
        .map(|v| v.to_string())
        .unwrap_or(path.to_string());

    let duration = Duration::from_millis(
        audio.chunks.len() as u64 * PACKET_DELAY.as_millis() as u64,
    );

    Ok(Song {
        path: cf,
        hash: Song::hash(&name, size),
        name,
        duration,
        size,
        capacity: 0,
    })
}
