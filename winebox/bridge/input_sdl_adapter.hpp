#pragma once

#include <array>
#include <cstdint>
#include <string>
#include <libretro.h>

namespace winebox {

class SdlInputAdapter {
public:
    enum class ControllerMode { GenericKeyboard, MouseLeftStick, MouseRightStick, CustomMapper };

    SdlInputAdapter(retro_input_poll_t poll, retro_input_state_t state, unsigned width, unsigned height);
    void set_dimensions(unsigned width, unsigned height);
    void set_callbacks(retro_input_poll_t poll, retro_input_state_t state);
    void set_keyboard_callback_enabled(bool enabled);
    void set_controller_mode(ControllerMode mode);
    void set_config_file(std::string path);
    bool overlay_active() const noexcept;
    void render_overlay(std::uint32_t* pixels, unsigned width, unsigned height) const;
    void keyboard_event(bool down, unsigned keycode, std::uint32_t character, std::uint16_t modifiers);
    void poll_and_enqueue();
    void reset();

private:
    enum class OverlayMode { Closed, Menu, Mapper, MapperSlots, Keyboard, AssignKeyboard };

    void load_default_mapping();
    void load_mapping();
    void save_mapping() const;
    void sync_virtual_joystick();
    void update_sdl_modifiers();
    void sync_key_state(unsigned key);
    void sync_all_key_states();
    void handle_overlay_input(const std::array<bool, 16>& buttons,
                              const std::array<std::int16_t, 4>& axes);

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
    ControllerMode controller_mode_ = ControllerMode::GenericKeyboard;
    std::array<bool, RETROK_LAST> key_down_{};
    std::array<bool, RETROK_LAST> callback_key_down_{};
    std::array<bool, RETROK_LAST> mapped_key_down_{};
    std::array<bool, RETROK_LAST> output_key_down_{};
    std::array<bool, 24> release_gate_{};
    std::array<bool, 24> mapped_source_down_{};
    std::array<std::array<unsigned, 4>, 24> mapping_{};
    std::array<bool, 3> mapped_mouse_down_{};
    std::array<bool, 16> ui_button_previous_{};
    std::array<bool, 4> ui_axis_previous_{};
    OverlayMode overlay_mode_ = OverlayMode::Closed;
    std::string config_file_;
    unsigned menu_selection_ = 0;
    unsigned selected_control_ = 0;
    unsigned selected_action_slot_ = 0;
    unsigned keyboard_selection_ = 0;
    unsigned osk_latched_modifier_ = 0;
};

} // namespace winebox
