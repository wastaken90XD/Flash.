#include "audio_adapter.hpp"

#include <algorithm>
#include <cmath>
#include <cstring>
#include <limits>

namespace winebox {
namespace {

std::size_t bytes_per_sample(PcmFormat format) {
    switch (format) {
    case PcmFormat::unsigned_8:
        return 1;
    case PcmFormat::signed_16:
        return 2;
    case PcmFormat::signed_32:
    case PcmFormat::float_32:
        return 4;
    }
    return 0;
}

float decode_sample(const std::uint8_t* source, PcmFormat format) {
    switch (format) {
    case PcmFormat::unsigned_8:
        return (static_cast<int>(*source) - 128) / 128.0f;
    case PcmFormat::signed_16: {
        std::int16_t value = 0;
        std::memcpy(&value, source, sizeof(value));
        return static_cast<float>(value) / 32768.0f;
    }
    case PcmFormat::signed_32: {
        std::int32_t value = 0;
        std::memcpy(&value, source, sizeof(value));
        return static_cast<float>(static_cast<double>(value) / 2147483648.0);
    }
    case PcmFormat::float_32: {
        float value = 0.0f;
        std::memcpy(&value, source, sizeof(value));
        if (!std::isfinite(value)) {
            return 0.0f;
        }
        return std::clamp(value, -1.0f, 1.0f);
    }
    }
    return 0.0f;
}

std::int16_t encode_sample(float value) {
    const float clipped = std::clamp(value, -1.0f, 1.0f);
    const long rounded = std::lround(clipped * 32768.0f);
    return static_cast<std::int16_t>(std::clamp<long>(rounded, -32768, 32767));
}

} // namespace

std::vector<std::int16_t> convert_audio_chunk(const void* source,
                                               std::size_t byte_count,
                                               PcmFormat format,
                                               unsigned sample_rate,
                                               unsigned channels) {
    const std::size_t sample_bytes = bytes_per_sample(format);
    if (!source || !sample_bytes || !sample_rate || !channels) {
        return {};
    }
    if (channels > std::numeric_limits<std::size_t>::max() / sample_bytes) {
        return {};
    }
    const std::size_t frame_bytes = sample_bytes * channels;
    const std::size_t input_frames = byte_count / frame_bytes;
    if (!input_frames) {
        return {};
    }
    if (input_frames > (std::numeric_limits<std::size_t>::max() / 44100u)) {
        return {};
    }

    const std::size_t output_frames = std::max<std::size_t>(
        1, (input_frames * 44100u + sample_rate / 2u) / sample_rate);
    if (output_frames > std::numeric_limits<std::size_t>::max() / 2) {
        return {};
    }

    const auto* bytes = static_cast<const std::uint8_t*>(source);
    std::vector<std::int16_t> output(output_frames * 2);
    for (std::size_t out = 0; out < output_frames; ++out) {
        const double position = static_cast<double>(out) * sample_rate / 44100.0;
        const std::size_t first = std::min<std::size_t>(static_cast<std::size_t>(position), input_frames - 1);
        const std::size_t second = std::min(first + 1, input_frames - 1);
        const float fraction = static_cast<float>(position - static_cast<double>(first));
        for (unsigned channel = 0; channel < 2; ++channel) {
            const unsigned source_channel = channels == 1 ? 0 : channel;
            const auto* first_sample = bytes + first * frame_bytes + source_channel * sample_bytes;
            const auto* second_sample = bytes + second * frame_bytes + source_channel * sample_bytes;
            const float a = decode_sample(first_sample, format);
            const float b = decode_sample(second_sample, format);
            output[out * 2 + channel] = encode_sample(a + (b - a) * fraction);
        }
    }
    return output;
}

} // namespace winebox
