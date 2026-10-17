#include "audio_adapter.hpp"

#include <array>
#include <cassert>
#include <cmath>
#include <cstdint>
#include <limits>

int main() {
    const std::array<std::int16_t, 4> stereo = {-32768, 32767, 1234, -2345};
    const auto same_rate = winebox::convert_audio_chunk(stereo.data(), sizeof(stereo),
        winebox::PcmFormat::signed_16, 44100, 2);
    assert(same_rate.size() == stereo.size());
    assert(same_rate[0] == -32768 && same_rate[1] == 32767);
    assert(same_rate[2] == 1234 && same_rate[3] == -2345);

    const std::array<std::int16_t, 2> mono = {-32768, 32767};
    const auto upsampled = winebox::convert_audio_chunk(mono.data(), sizeof(mono),
        winebox::PcmFormat::signed_16, 22050, 1);
    assert(upsampled.size() == 8);
    assert(upsampled[0] == -32768 && upsampled[1] == -32768);
    assert(upsampled[2] == -1 && upsampled[3] == -1);
    assert(upsampled[4] == 32767 && upsampled[5] == 32767);
    assert(upsampled[6] == 32767 && upsampled[7] == 32767);

    const std::array<std::uint8_t, 2> unsigned_samples = {0, 255};
    const auto u8 = winebox::convert_audio_chunk(unsigned_samples.data(), sizeof(unsigned_samples),
        winebox::PcmFormat::unsigned_8, 44100, 1);
    assert(u8.size() == 4);
    assert(u8[0] == -32768 && u8[1] == -32768);
    assert(u8[2] == 32512 && u8[3] == 32512);

    const std::array<float, 2> float_samples = {std::numeric_limits<float>::quiet_NaN(), 2.0f};
    const auto floats = winebox::convert_audio_chunk(float_samples.data(), sizeof(float_samples),
        winebox::PcmFormat::float_32, 44100, 1);
    assert(floats.size() == 4);
    assert(floats[0] == 0 && floats[1] == 0);
    assert(floats[2] == 32767 && floats[3] == 32767);

    assert(winebox::convert_audio_chunk(nullptr, 0, winebox::PcmFormat::signed_16, 44100, 2).empty());
    assert(winebox::convert_audio_chunk(stereo.data(), sizeof(stereo), winebox::PcmFormat::signed_16, 0, 2).empty());
    return 0;
}
