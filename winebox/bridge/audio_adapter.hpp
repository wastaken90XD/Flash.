#pragma once

#include <cstddef>
#include <cstdint>
#include <vector>

namespace winebox {

enum class PcmFormat {
    unsigned_8,
    signed_16,
    signed_32,
    float_32,
};

// Converts one SDL output chunk to the libretro contract (44.1 kHz,
// interleaved stereo, signed 16-bit PCM). Channels beyond the first stereo pair
// are ignored; mono is duplicated to both outputs.
std::vector<std::int16_t> convert_audio_chunk(const void* source,
                                               std::size_t byte_count,
                                               PcmFormat format,
                                               unsigned sample_rate,
                                               unsigned channels);

} // namespace winebox
