#include "retro_host.hpp"

#include <algorithm>
#include <chrono>
#include <cstring>
#include <limits>

namespace winebox {

RetroHost::RetroHost(retro_video_refresh_t video,
                     retro_audio_sample_batch_t audio,
                     retro_input_poll_t input_poll,
                     retro_input_state_t input_state,
                     std::size_t max_audio_frames)
    : video_(video),
      audio_(audio),
      input_poll_(input_poll),
      input_state_(input_state),
      max_audio_frames_(max_audio_frames) {}

void RetroHost::set_video_callback(retro_video_refresh_t video) noexcept {
    video_ = video;
}

void RetroHost::set_audio_callback(retro_audio_sample_batch_t audio) noexcept {
    audio_ = audio;
}

void RetroHost::set_input_callbacks(retro_input_poll_t poll, retro_input_state_t state) noexcept {
    input_poll_ = poll;
    input_state_ = state;
}

bool RetroHost::publish_video(const std::uint32_t* pixels,
                              unsigned width,
                              unsigned height,
                              std::size_t source_pitch) {
    const auto copy_started = std::chrono::steady_clock::now();
    if (!pixels || width == 0 || height == 0 || width > kMaxDimension || height > kMaxDimension) {
        return false;
    }
    if (width > std::numeric_limits<std::size_t>::max() / sizeof(std::uint32_t)) {
        return false;
    }
    const std::size_t row_bytes = static_cast<std::size_t>(width) * sizeof(std::uint32_t);
    if (source_pitch < row_bytes || height > std::numeric_limits<std::size_t>::max() / width) {
        return false;
    }

    std::lock_guard<std::mutex> publish_lock(publish_mutex_);
    std::vector<std::uint32_t> copy;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        copy.swap(video_staging_pixels_);
    }
    copy.resize(static_cast<std::size_t>(width) * height);
    const auto* source = reinterpret_cast<const std::uint8_t*>(pixels);
    auto* destination = reinterpret_cast<std::uint8_t*>(copy.data());
    for (unsigned row = 0; row < height; ++row) {
        std::memcpy(destination + static_cast<std::size_t>(row) * row_bytes,
                    source + static_cast<std::size_t>(row) * source_pitch,
                    row_bytes);
    }

    {
        std::lock_guard<std::mutex> lock(mutex_);
        video_pixels_.swap(copy);
        video_staging_pixels_.swap(copy);
        video_width_ = width;
        video_height_ = height;
        video_pending_ = true;
    }
    video_publication_count_.fetch_add(1, std::memory_order_relaxed);
    const auto copy_nanoseconds = std::chrono::duration_cast<std::chrono::nanoseconds>(
        std::chrono::steady_clock::now() - copy_started).count();
    video_publication_time_nanoseconds_.fetch_add(static_cast<std::uint64_t>(copy_nanoseconds),
                                                  std::memory_order_relaxed);
    return true;
}

bool RetroHost::video_dimensions(unsigned& width, unsigned& height) const {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!has_presented_video_) {
        return false;
    }
    width = presented_width_;
    height = presented_height_;
    return true;
}

std::uint64_t RetroHost::video_publication_count() const noexcept {
    return video_publication_count_.load(std::memory_order_relaxed);
}

std::uint64_t RetroHost::video_publication_time_nanoseconds() const noexcept {
    return video_publication_time_nanoseconds_.load(std::memory_order_relaxed);
}

std::uint64_t RetroHost::video_non_black_pixel_count() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return static_cast<std::uint64_t>(std::count_if(presented_pixels_.begin(), presented_pixels_.end(),
        [](std::uint32_t pixel) { return (pixel & 0x00FFFFFFu) != 0; }));
}

bool RetroHost::present_video(VideoOverlayCallback overlay, void* overlay_context, bool overlay_active) {
    if (!video_) {
        return false;
    }

    const std::uint32_t* pixels = nullptr;
    unsigned width = 0;
    unsigned height = 0;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        if (video_pending_) {
            video_pixels_.swap(presented_pixels_);
            presented_width_ = video_width_;
            presented_height_ = video_height_;
            video_pending_ = false;
            has_presented_video_ = true;
        }
        if (!has_presented_video_ || presented_pixels_.empty()) {
            return false;
        }
        pixels = presented_pixels_.data();
        width = presented_width_;
        height = presented_height_;
    }

    if (overlay && overlay_active) {
        overlay_pixels_.assign(pixels, pixels + static_cast<std::size_t>(width) * height);
        overlay(overlay_context, overlay_pixels_.data(), width, height);
        pixels = overlay_pixels_.data();
    }
    video_(pixels, width, height, static_cast<std::size_t>(width) * sizeof(std::uint32_t));
    return true;
}

bool RetroHost::queue_audio(const std::int16_t* samples, std::size_t frames) {
    if (!samples || frames == 0 || frames > max_audio_frames_) {
        return false;
    }
    if (frames > std::numeric_limits<std::size_t>::max() / 2) {
        return false;
    }

    std::lock_guard<std::mutex> lock(mutex_);
    const std::size_t pending_frames = pending_audio_.size() / 2;
    if (frames > max_audio_frames_ - pending_frames) {
        return false;
    }
    pending_audio_.insert(pending_audio_.end(), samples, samples + frames * 2);
    return true;
}

std::size_t RetroHost::drain_audio(std::size_t max_frames) {
    if (!audio_ || max_frames == 0) {
        return 0;
    }

    std::vector<std::int16_t> batch;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        const std::size_t frames = std::min(max_frames, pending_audio_.size() / 2);
        if (frames == 0) {
            return 0;
        }
        batch.assign(pending_audio_.begin(), pending_audio_.begin() + static_cast<std::ptrdiff_t>(frames * 2));
        pending_audio_.erase(pending_audio_.begin(), pending_audio_.begin() + static_cast<std::ptrdiff_t>(frames * 2));
    }

    const std::size_t offered_frames = batch.size() / 2;
    const std::size_t accepted_frames = std::min(audio_(batch.data(), offered_frames), offered_frames);
    if (accepted_frames < offered_frames) {
        const auto first_unaccepted = batch.begin() + static_cast<std::ptrdiff_t>(accepted_frames * 2);
        std::lock_guard<std::mutex> lock(mutex_);
        pending_audio_.insert(pending_audio_.begin(), first_unaccepted, batch.end());
    }
    return accepted_frames;
}

std::size_t RetroHost::drain_audio_exact(std::size_t frames) {
    if (!audio_ || frames == 0 || frames > std::numeric_limits<std::size_t>::max() / 2) {
        return 0;
    }

    std::vector<std::int16_t> batch(frames * 2, 0);
    std::size_t guest_frames = 0;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        guest_frames = std::min(frames, pending_audio_.size() / 2);
        std::copy_n(pending_audio_.begin(), static_cast<std::ptrdiff_t>(guest_frames * 2), batch.begin());
        pending_audio_.erase(pending_audio_.begin(), pending_audio_.begin() + static_cast<std::ptrdiff_t>(guest_frames * 2));
    }

    const std::size_t accepted_frames = std::min(audio_(batch.data(), frames), frames);
    const std::size_t unaccepted_guest_frames = accepted_frames < guest_frames ? guest_frames - accepted_frames : 0;
    if (unaccepted_guest_frames) {
        const auto first = batch.begin() + static_cast<std::ptrdiff_t>(accepted_frames * 2);
        const auto last = batch.begin() + static_cast<std::ptrdiff_t>(guest_frames * 2);
        std::lock_guard<std::mutex> lock(mutex_);
        pending_audio_.insert(pending_audio_.begin(), first, last);
    }
    return accepted_frames;
}

void RetroHost::poll_input() const {
    if (input_poll_) {
        input_poll_();
    }
}

std::int16_t RetroHost::input_state(unsigned port,
                                    unsigned device,
                                    unsigned index,
                                    unsigned id) const {
    return input_state_ ? input_state_(port, device, index, id) : 0;
}

} // namespace winebox
