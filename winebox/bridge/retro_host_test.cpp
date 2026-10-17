#include "retro_host.hpp"

#include <array>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <vector>

namespace {

unsigned video_width = 0;
unsigned video_height = 0;
std::size_t video_pitch = 0;
std::vector<std::uint32_t> video_copy;
unsigned poll_count = 0;
std::size_t audio_limit = 0;
std::vector<std::int16_t> audio_copy;

void RETRO_CALLCONV video_refresh(const void* data, unsigned width, unsigned height, std::size_t pitch) {
    video_width = width;
    video_height = height;
    video_pitch = pitch;
    const auto* pixels = static_cast<const std::uint32_t*>(data);
    video_copy.assign(pixels, pixels + static_cast<std::size_t>(width) * height);
}

std::size_t RETRO_CALLCONV audio_batch(const std::int16_t* data, std::size_t frames) {
    const std::size_t accepted = frames < audio_limit ? frames : audio_limit;
    audio_copy.insert(audio_copy.end(), data, data + accepted * 2);
    return accepted;
}

void RETRO_CALLCONV input_poll() {
    ++poll_count;
}

std::int16_t RETRO_CALLCONV input_state(unsigned port, unsigned device, unsigned index, unsigned id) {
    return static_cast<std::int16_t>(port + device + index + id);
}

} // namespace

int main() {
    winebox::RetroHost host(video_refresh, audio_batch, input_poll, input_state, 4);

    // Padded source rows are copied into a tightly packed frontend frame.
    const std::array<std::uint32_t, 6> padded_pixels = {
        0x00112233, 0x00445566, 0x00ffffff,
        0x00778899, 0x00aabbcc, 0x00000000,
    };
    assert(host.publish_video(padded_pixels.data(), 2, 2, 3 * sizeof(std::uint32_t)));
    assert(host.present_video());
    assert(video_width == 2 && video_height == 2);
    assert(video_pitch == 2 * sizeof(std::uint32_t));
    assert((video_copy == std::vector<std::uint32_t>{0x00112233, 0x00445566, 0x00778899, 0x00aabbcc}));
    assert(host.video_publication_count() == 1);
    assert(host.video_non_black_pixel_count() == 4);
    assert(host.present_video()); // Re-present the last frame without another guest publish.
    assert((video_copy == std::vector<std::uint32_t>{0x00112233, 0x00445566, 0x00778899, 0x00aabbcc}));
    assert(!host.publish_video(nullptr, 2, 2, 2 * sizeof(std::uint32_t)));
    assert(!host.publish_video(padded_pixels.data(), 2, 2, sizeof(std::uint32_t)));

    // Input callbacks are reached synchronously and missing state callbacks
    // have a neutral value.
    host.poll_input();
    assert(poll_count == 1);
    assert(host.input_state(1, 2, 3, 4) == 10);

    // A short frontend audio write keeps the unaccepted tail queued.
    const std::array<std::int16_t, 6> samples = {1, 2, 3, 4, 5, 6};
    assert(host.queue_audio(samples.data(), 3));
    assert(!host.queue_audio(samples.data(), 2)); // bounded four-frame queue
    audio_limit = 1;
    assert(host.drain_audio(3) == 1);
    assert((audio_copy == std::vector<std::int16_t>{1, 2}));
    audio_limit = 8;
    assert(host.drain_audio(8) == 2);
    assert((audio_copy == std::vector<std::int16_t>{1, 2, 3, 4, 5, 6}));
    assert(host.drain_audio(8) == 0);
    audio_copy.clear();
    audio_limit = 8;
    assert(host.queue_audio(samples.data(), 1));
    assert(host.drain_audio_exact(3) == 3);
    assert((audio_copy == std::vector<std::int16_t>{1, 2, 0, 0, 0, 0}));
    assert(host.drain_audio_exact(2) == 2);
    assert((audio_copy == std::vector<std::int16_t>{1, 2, 0, 0, 0, 0, 0, 0, 0, 0}));
    assert(!host.queue_audio(nullptr, 1));
    assert(!host.queue_audio(samples.data(), 5));

    return 0;
}
