#pragma once

#include <array>
#include <cstdint>
#include <libretro.h>

namespace winebox {

class SdlInputAdapter {
public:
    SdlInputAdapter(retro_input_poll_t poll, retro_input_state_t state, unsigned width, unsigned height);
    void set_dimensions(unsigned width, unsigned height);
    void set_callbacks(retro_input_poll_t poll, retro_input_state_t state);
    void set_keyboard_callback_enabled(bool enabled);
    void keyboard_event(bool down, unsigned keycode, std::uint32_t character, std::uint16_t modifiers);
    void poll_and_enqueue();
    void reset();

private:
    retro_input_poll_t poll_;
    retro_input_state_t state_;
    unsigned width_;
    unsigned height_;
    int cursor_x_;
    int cursor_y_;
    bool left_down_ = false;
    bool right_down_ = false;
    bool middle_down_ = false;
    bool keyboard_callback_enabled_ = false;
    std::array<bool, RETROK_LAST> key_down_{};
};

} // namespace winebox
