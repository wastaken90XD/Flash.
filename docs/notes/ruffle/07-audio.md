# R8 — Audio Pipeline and AVM1 `Sound` (`07-audio.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `core/src/backend/audio/`, `core/src/backend/audio/mixer.rs`, `core/src/backend/audio/decoders.rs`, and `core/src/avm1/globals/sound.rs` at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. `AudioBackend` Trait and `AudioMixer` (`Confidence: High`)

- **`AudioBackend` (`core/src/backend/audio.rs:88-205`):**
  - Registers embedded `DefineSound` tags (`register_sound`) and external MP3 data (`register_mp3`), starts event sounds (`start_sound`), timeline stream sounds (`start_stream`), and NetStream/FLV substreams (`start_substream`), stops instances (`stop_sound`, `stop_all_sounds`), queries `get_sound_position` / `get_sound_duration` / `get_sound_peak` / `get_sample_history`, and applies 2x2 channel matrix transforms (`set_sound_transform`).
- **`AudioMixer` & `AudioMixerProxy` (`core/src/backend/audio/mixer.rs:58-76, 723-766, 1069-1167`):**
  - `AudioMixer` stores registered `Sound` entries in a `SlotMap<SoundHandle, Sound>` (`core/src/backend/audio/mixer.rs:60`) and active `SoundInstance`s in `Arc<Mutex<SlotMap<SoundInstanceHandle, SoundInstance>>>` (`core/src/backend/audio/mixer.rs:63`).
  - Uses `dasp` (`dasp::signal::Signal<Frame = [i16; 2]>`, `dasp::interpolate::linear::Linear`) to resample each active stream from its source sample rate (`5512`, `11025`, `22050`, `44100` Hz) to `output_sample_rate` (`44100` Hz), applies the 2x2 `[left_to_left, right_to_left]` / `[left_to_right, right_to_right]` transform plus master `volume`, tracks 1024-sample window peaks and circular history (`CircBuf`, `core/src/backend/audio/mixer.rs:16-49`), and writes interleaved stereo frames into `output_buffer: &mut [T]`.

---

## 2. Audio Decoders (`core/src/backend/audio/decoders.rs:58-96`) (`Confidence: High`)

| Compression (`swf::AudioCompression`) | Decoder File | Implementation Details |
|---|---|---|
| `Uncompressed` / `UncompressedUnknownEndian` | `core/src/backend/audio/decoders/pcm.rs:1-73` | 8-bit unsigned (`u8 -> i16` via `(b as i16 - 128) << 8`) or 16-bit signed little-endian PCM, mono or stereo |
| `Adpcm` | `core/src/backend/audio/decoders/adpcm.rs:1-246` | Flash SWF IMA-variant ADPCM with 2, 3, 4, or 5 bits per sample, resetting predictor/step index every 4096 samples |
| `Mp3` | `core/src/backend/audio/decoders/mp3.rs:1-201` | Symphonia (`symphonia-bundle-mp3` default) or `minimp3`; handles `seek_samples` encoder delay (`skip_sample_frames`, `core/src/backend/audio/mixer.rs:160-165`) |
| `Nellymoser` | `core/src/backend/audio/decoders/nellymoser.rs:1-42` | Wraps `nellymoser-rs` (`0.1.2`) for speech-rate mono streams |
| `Aac` | `core/src/backend/audio/decoders/aac.rs:1-124` | Wraps `symphonia-codec-aac` for MP4/FLV streams |
| `G711ALawPCM` / `G711MuLawPCM` | `core/src/backend/audio/decoders/g711.rs:1-203` | Lookup-table A-law and Mu-law 8-bit logarithmic PCM decoders |

---

## 3. Event Sounds vs Stream Sounds, Envelopes, Looping, and In/Out Points (`Confidence: High`)

- **Event vs Stream Sounds (`AudioManager`, `core/src/backend/audio.rs:297-550`):**
  - Enforces Flash Player's global cap of **`MAX_SOUNDS = 32` concurrent sound instances** (`core/src/backend/audio.rs:321, 465`).
  - **Event sounds (`StartSound` tag / `Sound.start()`):** Support `SoundEvent::Stop` (stops active instances of that `SoundHandle`), `SoundEvent::Start` (starts only if no instance of that `SoundHandle` is already playing — Flash's `syncNoMultiple` flag), and `SoundEvent::Event` (always starts a new instance up to `MAX_SOUNDS`).
  - **Stream sounds (`SoundStreamHead` / `SoundStreamBlock`):** `StandardStreamDecoder` (`core/src/backend/audio/decoders.rs:124-195`) uses `StreamTagReader` to scan a MovieClip's tag stream frame-by-frame, extracting `SoundStreamBlock` payloads (slicing off the 4-byte `sample_count` + `seek_samples` prefix for MP3 streams) and syncing playback to the MovieClip's timeline.
- **In/Out Points, Looping, and Envelopes (`EventSoundStream`, `core/src/backend/audio/mixer.rs:786-960`):**
  - `in_sample` and `out_sample` in `swf::SoundInfo` are always specified in **44,100 Hz sample units**, so `EventSoundStream::new_with_settings` divides them by `sample_divisor = 44100.0 / decoder.sample_rate()` and adds `skip_sample_frames` (`core/src/backend/audio/mixer.rs:804-812`)!
  - `next_loop()` seeks the decoder back to `start_sample_frame` `num_loops` times (`core/src/backend/audio/mixer.rs:828-836`).
  - `EnvelopeStream` (`core/src/backend/audio/mixer.rs:895-995`) linearly interpolates between `swf::SoundEnvelopePoint` keyframes (which are also in 44.1 kHz sample units).

---

## 4. AVM1 `Sound` Object (`core/src/avm1/globals/sound.rs:1-750`) (`Confidence: High`)

- **Owner Clip vs Global Sound (`core/src/avm1/globals/sound.rs:227-268, 414-436`):**
  - `new Sound(target)`: If `target` is `null` or `undefined`, the `Sound` object controls the **global sound transform** (`use_global_sound() == true`, `core/src/avm1/globals/sound.rs:240-242`). Otherwise `target` is coerced to a string path and resolved dynamically on each method call!
- **Lazy `position` and `duration` Getters (`core/src/avm1/globals/sound.rs:188-213`):**
  - `position` and `duration` are **not** defined on `Sound.prototype` (`PROTO_DECLS`, lines 377–397); instead, `Sound::set_sound` adds them as instance getter properties when a sound is attached or loaded, only if the instance does not already have a user-defined property of that name!
- **`onSoundComplete` Dispatch (`AudioManager::update_sounds`, `core/src/backend/audio.rs:344-390`):**
  - Once per frame in `Player::update_timers` / `audio_manager.update_sounds`, Ruffle polls `audio.is_sound_playing(instance)`. When an instance finishes, it records the final `position = duration` and queues an `ActionType::Method { object: avm1_object, name: "onSoundComplete", args: vec![] }` on `ActionQueue`!

---

## 5. Pulling 44.1 kHz Stereo `i16` Frames Per Tick for Libretro (`Confidence: High`)

- In libretro (`retro_run`), the core runs at the SWF's `frame_rate` (e.g. `fps`) and outputs `44100 / fps` stereo `i16` frames per tick via `retro_audio_sample_batch_t`.
- Both `swfr-audio::AudioMixer` and Ruffle's `AudioMixer::mix::<i16>(&mut pcm_buf)` operate synchronously on a caller-supplied `&mut [i16]` slice (`samples_per_frame * 2`), requiring zero background audio threads!

---

## 6. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — `new Sound()` initial transform, `duration` before/after `attachSound`, and `setVolume` (`core/src/avm1/globals/sound.rs:188-213, 377-397`):**
   - Verified against `tests/tests/swfs/avm1/sound/output.txt:1-25` (`sound.duration: undefined` before attach, `sound1.duration: 1452`, `sound2.duration: 907`).
2. **Claim 2 — Lazy `position` / `duration` property definition does not overwrite existing own properties (`core/src/avm1/globals/sound.rs:189-212`):**
   - Verified against `tests/tests/swfs/avm1/sound_duration_position_props/output.txt:1-25`, where setting `sound.duration = "x"` and `sound.position = "x"` before loading preserves `sound.duration == "x"` while `sound.getDuration()` returns `1045` and `onSoundComplete` fires when `getPosition() == 1045`.
3. **Claim 3 — `Sound.id3` and `onID3(true)` callback (`core/src/avm1/globals/sound.rs:293-374`):**
   - Verified against `tests/tests/swfs/avm1/sound_id3/output.txt:1-25` (`onID3 called: true, undefined`, `id3[TCON] = Some Genre`, `id3[genre] = Some Genre`).

---

## 7. What Could Not Be Verified (`Confidence: High`)

- Real-time audio device latency (`cpal` in `desktop/src/backends/audio.rs`) is not applicable to libretro's synchronous `retro_audio_sample_batch_t` model and was not tested.

---

## 8. Open Questions

1. Should `swfr-audio` convert `SoundInfo.in_sample` and `out_sample` from 44,100 Hz units to the sound's native sample rate (`44100.0 / sample_rate`, `core/src/backend/audio/mixer.rs:804-812`) when playing lower-sample-rate (`11025` / `22050` Hz) event sounds with trim points?
2. Should `swfr-player` fire `onSoundComplete` on AVM1 `Sound` objects when a sound instance finishes playing during `run_frame`?

---

## 9. Test Ideas for `swfr`

1. **`Sound.onSoundComplete` callback test:** Attach a short `DefineSound` via `s = new Sound(); s.attachSound("beep"); s.start();`, step frames until completion, and assert `s.onSoundComplete` is invoked and `s.position == s.duration`.
2. **44.1 kHz `in_sample` / `out_sample` scaling test:** Play a 22,050 Hz sound with `in_sample = 22050` (0.5 s in 44.1 kHz units) and verify playback starts at sample `11025` of the 22.05 kHz buffer.
3. **`MAX_SOUNDS = 32` cap and `syncNoMultiple` test:** Verify starting a sound with `no_multiple = true` while already playing does not spawn a duplicate instance.
