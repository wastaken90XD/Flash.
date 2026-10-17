#pragma once

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <mutex>
#include <vector>

#include <libretro.h>

namespace winebox {

// Thread-safe software-output boundary for the first Winebox integration.
// BoxedWine worker threads publish frames/audio here; the libretro caller
// invokes callbacks from the frontend thread via present_video()/drain_audio().
class RetroHost {
public:
    explicit RetroHost(retro_video_refresh_t video,
                       retro_audio_sample_batch_t audio,
                       retro_input_poll_t input_poll,
                       retro_input_state_t input_state,
                       std::size_t max_audio_frames = 88200);

    // Copies XRGB8888 rows into host-owned storage. source_pitch is in bytes.
    // Returns false for invalid dimensions, pitch, or a null source.
    bool publish_video(const std::uint32_t* pixels,
                       unsigned width,
                       unsigned height,
                       std::size_t source_pitch);

    // Frontend callbacks can be installed or replaced after retro_init().
    void set_video_callback(retro_video_refresh_t video) noexcept;
    void set_audio_callback(retro_audio_sample_batch_t audio) noexcept;
    void set_input_callbacks(retro_input_poll_t poll, retro_input_state_t state) noexcept;

    // Called from the libretro thread. Double-buffer swapping keeps the frame
    // stable during the callback without copying it a second time.
    bool present_video();
    bool video_dimensions(unsigned& width, unsigned& height) const;
    std::uint64_t video_publication_count() const noexcept;
    std::uint64_t video_publication_time_nanoseconds() const noexcept;
    std::uint64_t video_non_black_pixel_count() const;

    // Adds interleaved stereo signed-16-bit frames to a bounded queue.
    // Returns false for null data, zero frames, or queue-capacity overflow.
    bool queue_audio(const std::int16_t* samples, std::size_t frames);

    // Sends up to max_frames to the frontend. If the callback accepts fewer
    // frames, the unaccepted tail remains queued for the next retro_run().
    std::size_t drain_audio(std::size_t max_frames);

    // Sends exactly frames to the frontend, padding missing guest audio with
    // silence. Any unaccepted guest samples are retained for the next call.
    std::size_t drain_audio_exact(std::size_t frames);

    // Input is polled synchronously from the frontend thread. These wrappers
    // never call frontend callbacks from an emulator worker thread.
    void poll_input() const;
    std::int16_t input_state(unsigned port,
                             unsigned device,
                             unsigned index,
                             unsigned id) const;

private:
    static constexpr unsigned kMaxDimension = 4096;

    retro_video_refresh_t video_;
    retro_audio_sample_batch_t audio_;
    retro_input_poll_t input_poll_;
    retro_input_state_t input_state_;
    const std::size_t max_audio_frames_;

    mutable std::mutex mutex_;
    std::vector<std::uint32_t> video_pixels_;
    std::vector<std::uint32_t> presented_pixels_;
    unsigned video_width_ = 0;
    unsigned video_height_ = 0;
    unsigned presented_width_ = 0;
    unsigned presented_height_ = 0;
    bool video_pending_ = false;
    bool has_presented_video_ = false;
    std::atomic<std::uint64_t> video_publication_count_{0};
    std::atomic<std::uint64_t> video_publication_time_nanoseconds_{0};
    std::vector<std::int16_t> pending_audio_;
};

} // namespace winebox
