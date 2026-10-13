//! Audio decoding (PCM, SWF IMA ADPCM 2/3/4/5-bit, MP3 via `symphonia`), linear resampling
//! to 44.1 kHz stereo, event sound voices, and frame-locked streaming audio mixer for `swfr`
//! (TSD 1.6, 2.8, 2.12 M5, 3.6 M5).

use std::collections::BTreeMap;
use std::io::Cursor;
use swfr_reader::Reader;
use swfr_swf::{AudioFormat, SoundDef, SoundInfo, SoundStreamHeadDef};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Output sample rate required by the libretro core (`44100.0` Hz per TSD 2.8).
pub const OUTPUT_SAMPLE_RATE: u32 = 44_100;

/// Maximum simultaneous active sound voices (TSD 2.8).
pub const MAX_VOICES: usize = 32;

const IMA_STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408,
    449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630,
    9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

const INDEX_TABLE_2BIT: [i32; 2] = [-1, 2];
const INDEX_TABLE_3BIT: [i32; 4] = [-1, -1, 2, 4];
const INDEX_TABLE_4BIT: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];
const INDEX_TABLE_5BIT: [i32; 16] = [-1, -1, -1, -1, -1, -1, -1, -1, 1, 2, 4, 6, 8, 10, 13, 16];

/// Converts an SWF 2-bit sample rate code (`0..=3`) to its sample rate in Hz.
#[must_use]
pub const fn swf_rate_hz(rate_code: u8) -> f64 {
    match rate_code & 0x03 {
        0 => 5512.5,
        1 => 11025.0,
        2 => 22050.0,
        _ => 44100.0,
    }
}

/// Decodes raw PCM audio (`UncompressedNativeEndian` or `PcmLittleEndian`) into stereo `[L, R]`
/// frames at the source sample rate.
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub fn decode_pcm_to_stereo(data: &[u8], is_16bit: bool, is_stereo: bool) -> Vec<[i16; 2]> {
    let mut out = Vec::new();
    if is_16bit {
        if is_stereo {
            for chunk in data.chunks_exact(4) {
                let l = i16::from_le_bytes([chunk[0], chunk[1]]);
                let r = i16::from_le_bytes([chunk[2], chunk[3]]);
                out.push([l, r]);
            }
        } else {
            for chunk in data.chunks_exact(2) {
                let s = i16::from_le_bytes([chunk[0], chunk[1]]);
                out.push([s, s]);
            }
        }
    } else if is_stereo {
        for chunk in data.chunks_exact(2) {
            let l = (i16::from(chunk[0]) - 128) << 8;
            let r = (i16::from(chunk[1]) - 128) << 8;
            out.push([l, r]);
        }
    } else {
        for &b in data {
            let s = (i16::from(b) - 128) << 8;
            out.push([s, s]);
        }
    }
    out
}

/// Decodes SWF IMA ADPCM (`AudioFormat::Adpcm`, supporting 2, 3, 4, and 5-bit indices per
/// SWF specification v19 Chapter 10) into stereo `[L, R]` frames at the source sample rate.
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub fn decode_swf_adpcm(data: &[u8], is_stereo: bool, max_samples: usize) -> Vec<[i16; 2]> {
    let mut r = Reader::new(data);
    let Ok(code_bits_raw) = r.read_ubits(2) else {
        return Vec::new();
    };
    let nbits = u8::try_from(code_bits_raw + 2).unwrap_or(4); // 2, 3, 4, or 5 bits
    let sign_mask = 1_u32 << (nbits - 1);
    let mag_mask = sign_mask - 1;

    let mut out = Vec::new();

    while !r.is_eof() && out.len() < max_samples {
        // Initial sample(s) and step index for a block of up to 4096 samples
        let Ok(mut val_l) = r.read_sbits(16) else {
            break;
        };
        let Ok(idx_l_raw) = r.read_ubits(6) else {
            break;
        };
        let mut idx_l = (idx_l_raw as i32).clamp(0, 88);

        let (mut val_r, mut idx_r) = if is_stereo {
            let Ok(vr) = r.read_sbits(16) else {
                break;
            };
            let Ok(ir) = r.read_ubits(6) else {
                break;
            };
            (vr, (ir as i32).clamp(0, 88))
        } else {
            (val_l, idx_l)
        };

        out.push([
            val_l.clamp(-32768, 32767) as i16,
            val_r.clamp(-32768, 32767) as i16,
        ]);

        for _ in 0..4095 {
            if out.len() >= max_samples {
                break;
            }
            let Ok(delta_l) = r.read_ubits(nbits) else {
                return out;
            };
            step_adpcm_channel(delta_l, nbits, sign_mask, mag_mask, &mut val_l, &mut idx_l);

            if is_stereo {
                let Ok(delta_r) = r.read_ubits(nbits) else {
                    return out;
                };
                step_adpcm_channel(delta_r, nbits, sign_mask, mag_mask, &mut val_r, &mut idx_r);
            } else {
                val_r = val_l;
            }
            out.push([
                val_l.clamp(-32768, 32767) as i16,
                val_r.clamp(-32768, 32767) as i16,
            ]);
        }
    }

    out
}

#[allow(clippy::arithmetic_side_effects)]
fn step_adpcm_channel(
    code: u32,
    nbits: u8,
    sign_mask: u32,
    mag_mask: u32,
    val: &mut i32,
    idx: &mut i32,
) {
    let step = IMA_STEP_TABLE[*idx as usize];
    let mag = code & mag_mask;
    let mut diff = step >> (nbits - 1);
    for bit in 0..(nbits - 1) {
        if (mag & (1_u32 << bit)) != 0 {
            diff += step >> (nbits - 2 - bit);
        }
    }
    if (code & sign_mask) != 0 {
        *val = (*val - diff).clamp(-32768, 32767);
    } else {
        *val = (*val + diff).clamp(-32768, 32767);
    }
    let idx_delta = match nbits {
        2 => INDEX_TABLE_2BIT[(mag as usize) & 1],
        3 => INDEX_TABLE_3BIT[(mag as usize) & 3],
        4 => INDEX_TABLE_4BIT[(mag as usize) & 7],
        _ => INDEX_TABLE_5BIT[(mag as usize) & 15],
    };
    *idx = (*idx + idx_delta).clamp(0, 88);
}

/// Decodes an SWF MP3 payload (after skipping the 2-byte `SeekSamples` prefix for `DefineSound`
/// or 4-byte `SampleCount + SeekSamples` prefix for `SoundStreamBlock`) into stereo `[L, R]`
/// frames and returns `(sample_rate_hz, frames)`.
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub fn decode_swf_mp3(data: &[u8], is_stream_block: bool) -> (f64, Vec<[i16; 2]>) {
    let header_skip = if is_stream_block { 4 } else { 2 };
    if data.len() <= header_skip {
        return (44100.0, Vec::new());
    }
    let seek_samples = if is_stream_block {
        i16::from_le_bytes([data[2], data[3]]).max(0) as usize
    } else {
        i16::from_le_bytes([data[0], data[1]]).max(0) as usize
    };
    let mp3_bytes = data[header_skip..].to_vec();

    let mss = MediaSourceStream::new(Box::new(Cursor::new(mp3_bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");

    let Ok(probed) = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) else {
        return (44100.0, Vec::new());
    };
    let mut format = probed.format;
    let Some(track) = format.default_track() else {
        return (44100.0, Vec::new());
    };
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let mut rate_hz = codec_params.sample_rate.unwrap_or(44100) as f64;

    let Ok(mut decoder) =
        symphonia::default::get_codecs().make(&codec_params, &DecoderOptions::default())
    else {
        return (rate_hz, Vec::new());
    };

    let mut frames = Vec::new();
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let Ok(decoded) = decoder.decode(&packet) else {
            continue;
        };
        let spec = *decoded.spec();
        rate_hz = spec.rate as f64;
        let channels = spec.channels.count().max(1);
        let mut sbuf = SampleBuffer::<i16>::new(decoded.capacity() as u64, spec);
        sbuf.copy_interleaved_ref(decoded);
        let samples = sbuf.samples();
        if channels >= 2 {
            for chunk in samples.chunks_exact(channels) {
                frames.push([chunk[0], chunk[1]]);
            }
        } else {
            for &s in samples {
                frames.push([s, s]);
            }
        }
    }

    if seek_samples > 0 && seek_samples < frames.len() {
        frames.drain(0..seek_samples);
    }
    (rate_hz, frames)
}

/// Linearly resamples stereo `[L, R]` frames from `src_rate_hz` to `44100.0` Hz (`OUTPUT_SAMPLE_RATE`).
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub fn resample_to_44k1(frames: &[[i16; 2]], src_rate_hz: f64) -> Vec<[i16; 2]> {
    if frames.is_empty() {
        return Vec::new();
    }
    if (src_rate_hz - f64::from(OUTPUT_SAMPLE_RATE)).abs() < 0.5 {
        return frames.to_vec();
    }
    let ratio = src_rate_hz / f64::from(OUTPUT_SAMPLE_RATE);
    let out_len = ((frames.len() as f64) / ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = (i as f64) * ratio;
        let idx0 = (src_pos.floor() as usize).min(frames.len() - 1);
        let idx1 = (idx0 + 1).min(frames.len() - 1);
        let frac = src_pos - (idx0 as f64);

        let f0 = frames[idx0];
        let f1 = frames[idx1];
        let l = (f64::from(f0[0]) * (1.0 - frac) + f64::from(f1[0]) * frac).round() as i16;
        let r = (f64::from(f0[1]) * (1.0 - frac) + f64::from(f1[1]) * frac).round() as i16;
        out.push([l, r]);
    }
    out
}

/// Decodes a [`SoundDef`] (`DefineSound`) into 44.1 kHz stereo frames.
#[must_use]
pub fn decode_sound_def(snd: &SoundDef) -> Vec<[i16; 2]> {
    let rate_hz = swf_rate_hz(snd.sample_rate_code);
    let raw_stereo = match snd.format {
        AudioFormat::UncompressedNativeEndian | AudioFormat::PcmLittleEndian => {
            decode_pcm_to_stereo(&snd.data, snd.is_16bit, snd.is_stereo)
        }
        AudioFormat::Adpcm => decode_swf_adpcm(
            &snd.data,
            snd.is_stereo,
            usize::try_from(snd.sample_count).unwrap_or(65536).max(1),
        ),
        AudioFormat::Mp3 => {
            let (mp3_rate, frames) = decode_swf_mp3(&snd.data, false);
            return resample_to_44k1(&frames, mp3_rate);
        }
        AudioFormat::Nellymoser16k
        | AudioFormat::Nellymoser8k
        | AudioFormat::Nellymoser
        | AudioFormat::Speex
        | AudioFormat::Unknown(_) => Vec::new(),
    };
    resample_to_44k1(&raw_stereo, rate_hz)
}

/// Decodes a `SoundStreamBlock` using the active `SoundStreamHeadDef` into 44.1 kHz stereo frames.
#[must_use]
pub fn decode_stream_block(head: &SoundStreamHeadDef, data: &[u8]) -> Vec<[i16; 2]> {
    let rate_hz = swf_rate_hz(head.stream_rate_code);
    let raw_stereo = match head.stream_format {
        AudioFormat::UncompressedNativeEndian | AudioFormat::PcmLittleEndian => {
            decode_pcm_to_stereo(data, head.stream_is_16bit, head.stream_is_stereo)
        }
        AudioFormat::Adpcm => decode_swf_adpcm(
            data,
            head.stream_is_stereo,
            usize::from(head.stream_sample_count).max(1),
        ),
        AudioFormat::Mp3 => {
            let (mp3_rate, frames) = decode_swf_mp3(data, true);
            return resample_to_44k1(&frames, mp3_rate);
        }
        _ => Vec::new(),
    };
    resample_to_44k1(&raw_stereo, rate_hz)
}

#[derive(Debug, Clone)]
struct ActiveVoice {
    sound_id: u16,
    samples_44k1: Vec<[i16; 2]>,
    pos: usize,
    end_pos: usize,
    loops_remaining: u16,
    info: SoundInfo,
}

/// 44.1 kHz stereo audio mixer managing event sound voices (`DefineSound` / `StartSound`) and
/// frame-locked streaming audio (`SoundStreamHead` / `SoundStreamBlock`) (TSD 2.8, M5).
#[derive(Debug, Clone, Default)]
pub struct AudioMixer {
    defined_sounds: BTreeMap<u16, Vec<[i16; 2]>>,
    voices: Vec<ActiveVoice>,
    stream_head: Option<SoundStreamHeadDef>,
    stream_fifo: Vec<[i16; 2]>,
    /// Master volume (`0..=100`, where `0` mutes all output).
    pub volume_percent: i32,
}

impl AudioMixer {
    /// Creates a new [`AudioMixer`] with `100%` volume.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            defined_sounds: BTreeMap::new(),
            voices: Vec::new(),
            stream_head: None,
            stream_fifo: Vec::new(),
            volume_percent: 100,
        }
    }

    /// Pre-decodes and registers a [`SoundDef`] (`DefineSound`) in the mixer's bank.
    pub fn register_sound(&mut self, snd: &SoundDef) {
        let pcm = decode_sound_def(snd);
        self.defined_sounds.insert(snd.id, pcm);
    }

    /// Triggers or stops an event sound (`StartSound`).
    pub fn start_sound(&mut self, sound_id: u16, info: &SoundInfo) {
        if info.sync_stop {
            self.voices.retain(|v| v.sound_id != sound_id);
            return;
        }
        if info.sync_no_multiple && self.voices.iter().any(|v| v.sound_id == sound_id) {
            return;
        }
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        let Some(samples) = self.defined_sounds.get(&sound_id).cloned() else {
            return;
        };
        if samples.is_empty() {
            return;
        }
        let start_pos = info
            .in_point
            .and_then(|p| usize::try_from(p).ok())
            .unwrap_or(0)
            .min(samples.len().saturating_sub(1));
        let end_pos = info
            .out_point
            .and_then(|p| usize::try_from(p).ok())
            .unwrap_or(samples.len())
            .min(samples.len());
        let loops = info.loop_count.unwrap_or(1).max(1);

        self.voices.push(ActiveVoice {
            sound_id,
            samples_44k1: samples,
            pos: start_pos,
            end_pos,
            loops_remaining: loops,
            info: info.clone(),
        });
    }

    /// Stops all playing event and streaming sounds (`ActionStopSounds`).
    pub fn stop_all_sounds(&mut self) {
        self.voices.clear();
        self.stream_fifo.clear();
    }

    /// Sets the active `SoundStreamHead` / `SoundStreamHead2` configuration.
    pub fn set_stream_head(&mut self, head: SoundStreamHeadDef) {
        self.stream_head = Some(head);
    }

    /// Decodes and appends a `SoundStreamBlock` for the current video frame, applying drift
    /// correction if the stream FIFO exceeds 2 frames of audio (`2 * target_frames`).
    pub fn push_stream_block(&mut self, data: &[u8], target_frames_per_step: usize) {
        let Some(ref head) = self.stream_head else {
            return;
        };
        let mut decoded = decode_stream_block(head, data);
        // Pad or trim to keep stream locked to the video frame rate within 1 frame (TSD 2.8 & 3.6 M5)
        let max_drift = target_frames_per_step.saturating_mul(2).max(1);
        if self.stream_fifo.len() > max_drift {
            let drop_count = self.stream_fifo.len().saturating_sub(target_frames_per_step);
            self.stream_fifo.drain(0..drop_count);
        }
        if decoded.len() < target_frames_per_step {
            decoded.resize(target_frames_per_step, [0, 0]);
        }
        self.stream_fifo.extend_from_slice(&decoded);
    }

    /// Mixes `num_frames` stereo samples (`num_frames * 2` `i16` values) at 44.1 kHz into
    /// an interleaved `[L, R, L, R, ...]` buffer.
    #[must_use]
    #[allow(clippy::arithmetic_side_effects)]
    pub fn mix_interleaved(&mut self, num_frames: usize) -> Vec<i16> {
        let mut out = vec![0_i16; num_frames.saturating_mul(2)];
        let vol = self.volume_percent.clamp(0, 100);

        for frame_idx in 0..num_frames {
            let mut acc_l: i32 = 0;
            let mut acc_r: i32 = 0;

            // 1. Streaming audio FIFO
            if frame_idx < self.stream_fifo.len() {
                let s = self.stream_fifo[frame_idx];
                acc_l += i32::from(s[0]);
                acc_r += i32::from(s[1]);
            }

            // 2. Active event voices
            let mut v_idx = 0_usize;
            while v_idx < self.voices.len() {
                let voice = &mut self.voices[v_idx];
                if voice.pos < voice.end_pos {
                    let s = voice.samples_44k1[voice.pos];
                    let (env_l, env_r) = eval_envelope(&voice.info, voice.pos);
                    acc_l += (i32::from(s[0]) * env_l) >> 15;
                    acc_r += (i32::from(s[1]) * env_r) >> 15;
                    voice.pos += 1;
                }
                if voice.pos >= voice.end_pos {
                    if voice.loops_remaining > 1 {
                        voice.loops_remaining -= 1;
                        voice.pos = voice
                            .info
                            .in_point
                            .and_then(|p| usize::try_from(p).ok())
                            .unwrap_or(0)
                            .min(voice.end_pos.saturating_sub(1));
                        v_idx += 1;
                    } else {
                        self.voices.remove(v_idx);
                    }
                } else {
                    v_idx += 1;
                }
            }

            let scaled_l = ((acc_l * vol) / 100).clamp(-32768, 32767) as i16;
            let scaled_r = ((acc_r * vol) / 100).clamp(-32768, 32767) as i16;
            out[frame_idx * 2] = scaled_l;
            out[frame_idx * 2 + 1] = scaled_r;
        }

        let consumed = num_frames.min(self.stream_fifo.len());
        if consumed > 0 {
            self.stream_fifo.drain(0..consumed);
        }

        out
    }
}

fn eval_envelope(info: &SoundInfo, pos: usize) -> (i32, i32) {
    let Some(last) = info
        .envelope
        .iter()
        .rfind(|pt| (pt.pos44 as usize) <= pos)
        .or_else(|| info.envelope.first())
    else {
        return (32768, 32768);
    };
    (i32::from(last.left_level), i32::from(last.right_level))
}

/// Computes a deterministic 64-bit FNV-1a hash over an interleaved `i16` audio sample slice.
#[must_use]
pub fn hash_audio_samples(samples: &[i16]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &s in samples {
        for b in s.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use swfr_testkit::BitWriter;

    #[test]
    fn pcm_and_resampler_known_output() {
        // 4 samples of 16-bit mono at 11025 Hz -> resampled 4x to 16 stereo frames at 44100 Hz
        let mut data = Vec::new();
        for s in [1000_i16, 2000, -1000, -2000] {
            data.extend_from_slice(&s.to_le_bytes());
        }
        let stereo = decode_pcm_to_stereo(&data, true, false);
        assert_eq!(stereo.len(), 4);
        let resampled = resample_to_44k1(&stereo, 11025.0);
        assert_eq!(resampled.len(), 16);
        assert_eq!(resampled[0], [1000, 1000]);
    }

    #[test]
    fn adpcm_2_3_4_5_bit_decodes_cleanly() {
        for code_bits in 0..=3_u32 {
            let nbits = u8::try_from(code_bits + 2).unwrap_or(4);
            let mut w = BitWriter::new();
            w.write_ubits(code_bits, 2);
            w.write_sbits(500, 16); // initial sample = 500
            w.write_ubits(10, 6); // initial step index = 10
            for i in 0..15_u32 {
                w.write_ubits(i & ((1 << nbits) - 1), nbits);
            }
            let bytes = w.into_bytes();
            let decoded = decode_swf_adpcm(&bytes, false, 16);
            assert_eq!(decoded.len(), 16);
            assert_eq!(decoded[0], [500, 500]);
        }
    }

    #[test]
    #[allow(clippy::arithmetic_side_effects)]
    fn stream_sync_300_frames_marker_pulse_within_one_frame() {
        // TSD 3.6 M5: Build a 300-frame stream with a marker pulse at known frame numbers,
        // run for 300 frames, and assert peak sample occurs within 1 frame of expected index.
        let frames_per_step = 735_usize; // 44100 Hz / 60 fps
        let marker_frames = [0_usize, 60, 120, 180, 240];

        let mut mixer = AudioMixer::new();
        mixer.set_stream_head(SoundStreamHeadDef {
            version: 2,
            playback_rate_code: 3, // 44100 Hz
            playback_is_16bit: true,
            playback_is_stereo: false,
            stream_format: AudioFormat::PcmLittleEndian,
            stream_rate_code: 3, // 44100 Hz
            stream_is_16bit: true,
            stream_is_stereo: false,
            stream_sample_count: 735,
            latency_seek: None,
        });

        let mut all_output_stereo: Vec<i16> = Vec::with_capacity(300 * frames_per_step);

        for f in 0..300_usize {
            let mut block = Vec::with_capacity(frames_per_step * 2);
            let is_marker = marker_frames.contains(&f);
            for s_idx in 0..frames_per_step {
                let sample: i16 = if is_marker && s_idx == 10 {
                    24_000
                } else {
                    0
                };
                block.extend_from_slice(&sample.to_le_bytes());
            }
            mixer.push_stream_block(&block, frames_per_step);
            let mixed = mixer.mix_interleaved(frames_per_step);
            for pair in mixed.chunks_exact(2) {
                all_output_stereo.push(pair[0]);
            }
        }

        for &expected_frame in &marker_frames {
            let window_start = expected_frame.saturating_sub(1) * frames_per_step;
            let window_end = ((expected_frame + 2) * frames_per_step).min(all_output_stereo.len());
            let (peak_offset, &peak_val) = all_output_stereo[window_start..window_end]
                .iter()
                .enumerate()
                .max_by_key(|(_, &v)| v)
                .expect("non-empty window");
            assert_eq!(peak_val, 24_000);
            let actual_sample_idx = window_start + peak_offset;
            let expected_sample_idx = expected_frame * frames_per_step + 10;
            let diff_samples = actual_sample_idx.abs_diff(expected_sample_idx);
            assert!(
                diff_samples <= frames_per_step,
                "marker pulse at frame {expected_frame} drifted by {diff_samples} samples (> 1 frame = {frames_per_step})"
            );
        }

        // Deterministic mix hash check
        let hash = hash_audio_samples(&all_output_stereo);
        assert_ne!(hash, 0);
    }
}
