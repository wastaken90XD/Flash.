#include "input_sdl_adapter.hpp"

#include <SDL.h>

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdlib>

namespace winebox {
namespace {

SDL_Keycode keycode_for_retro(unsigned key) {
    if ((key >= RETROK_SPACE && key <= RETROK_AT) ||
        (key >= RETROK_LEFTBRACKET && key <= RETROK_BACKQUOTE) ||
        (key >= RETROK_a && key <= RETROK_z)) {
        return static_cast<SDL_Keycode>(key);
    }
    switch (key) {
    case RETROK_BACKSPACE: return SDLK_BACKSPACE;
    case RETROK_TAB: return SDLK_TAB;
    case RETROK_CLEAR: return SDLK_CLEAR;
    case RETROK_RETURN: return SDLK_RETURN;
    case RETROK_PAUSE: return SDLK_PAUSE;
    case RETROK_ESCAPE: return SDLK_ESCAPE;
    case RETROK_DELETE: return SDLK_DELETE;
    case RETROK_KP0: return SDLK_KP_0;
    case RETROK_KP1: return SDLK_KP_1;
    case RETROK_KP2: return SDLK_KP_2;
    case RETROK_KP3: return SDLK_KP_3;
    case RETROK_KP4: return SDLK_KP_4;
    case RETROK_KP5: return SDLK_KP_5;
    case RETROK_KP6: return SDLK_KP_6;
    case RETROK_KP7: return SDLK_KP_7;
    case RETROK_KP8: return SDLK_KP_8;
    case RETROK_KP9: return SDLK_KP_9;
    case RETROK_KP_PERIOD: return SDLK_KP_PERIOD;
    case RETROK_KP_DIVIDE: return SDLK_KP_DIVIDE;
    case RETROK_KP_MULTIPLY: return SDLK_KP_MULTIPLY;
    case RETROK_KP_MINUS: return SDLK_KP_MINUS;
    case RETROK_KP_PLUS: return SDLK_KP_PLUS;
    case RETROK_KP_ENTER: return SDLK_KP_ENTER;
    case RETROK_KP_EQUALS: return SDLK_KP_EQUALS;
    case RETROK_UP: return SDLK_UP;
    case RETROK_DOWN: return SDLK_DOWN;
    case RETROK_RIGHT: return SDLK_RIGHT;
    case RETROK_LEFT: return SDLK_LEFT;
    case RETROK_INSERT: return SDLK_INSERT;
    case RETROK_HOME: return SDLK_HOME;
    case RETROK_END: return SDLK_END;
    case RETROK_PAGEUP: return SDLK_PAGEUP;
    case RETROK_PAGEDOWN: return SDLK_PAGEDOWN;
    case RETROK_F1: return SDLK_F1;
    case RETROK_F2: return SDLK_F2;
    case RETROK_F3: return SDLK_F3;
    case RETROK_F4: return SDLK_F4;
    case RETROK_F5: return SDLK_F5;
    case RETROK_F6: return SDLK_F6;
    case RETROK_F7: return SDLK_F7;
    case RETROK_F8: return SDLK_F8;
    case RETROK_F9: return SDLK_F9;
    case RETROK_F10: return SDLK_F10;
    case RETROK_F11: return SDLK_F11;
    case RETROK_F12: return SDLK_F12;
    case RETROK_F13: return SDLK_F13;
    case RETROK_F14: return SDLK_F14;
    case RETROK_F15: return SDLK_F15;
    case RETROK_NUMLOCK: return SDLK_NUMLOCKCLEAR;
    case RETROK_CAPSLOCK: return SDLK_CAPSLOCK;
    case RETROK_SCROLLOCK: return SDLK_SCROLLLOCK;
    case RETROK_RSHIFT: return SDLK_RSHIFT;
    case RETROK_LSHIFT: return SDLK_LSHIFT;
    case RETROK_RCTRL: return SDLK_RCTRL;
    case RETROK_LCTRL: return SDLK_LCTRL;
    case RETROK_RALT: return SDLK_RALT;
    case RETROK_LALT: return SDLK_LALT;
    case RETROK_RMETA: return SDLK_RGUI;
    case RETROK_LMETA: return SDLK_LGUI;
    case RETROK_LSUPER: return SDLK_LGUI;
    case RETROK_RSUPER: return SDLK_RGUI;
    case RETROK_MODE: return SDLK_MODE;
    case RETROK_HELP: return SDLK_HELP;
    case RETROK_PRINT: return SDLK_PRINTSCREEN;
    case RETROK_MENU: return SDLK_MENU;
    default: return SDLK_UNKNOWN;
    }
}

SDL_Keymod modifiers_for_retro(std::uint16_t modifiers) {
    SDL_Keymod result = KMOD_NONE;
    if (modifiers & RETROKMOD_SHIFT) result = static_cast<SDL_Keymod>(result | KMOD_SHIFT);
    if (modifiers & RETROKMOD_CTRL) result = static_cast<SDL_Keymod>(result | KMOD_CTRL);
    if (modifiers & RETROKMOD_ALT) result = static_cast<SDL_Keymod>(result | KMOD_ALT);
    if (modifiers & RETROKMOD_META) result = static_cast<SDL_Keymod>(result | KMOD_GUI);
    if (modifiers & RETROKMOD_CAPSLOCK) result = static_cast<SDL_Keymod>(result | KMOD_CAPS);
    if (modifiers & RETROKMOD_NUMLOCK) result = static_cast<SDL_Keymod>(result | KMOD_NUM);
    return result;
}

void push_text_input(std::uint32_t codepoint) {
    if (codepoint == 0 || codepoint > 0x10FFFF || (codepoint >= 0xD800 && codepoint <= 0xDFFF))
        return;

    SDL_Event event{};
    event.type = SDL_TEXTINPUT;
    char* text = event.text.text;
    if (codepoint <= 0x7F) {
        text[0] = static_cast<char>(codepoint);
    } else if (codepoint <= 0x7FF) {
        text[0] = static_cast<char>(0xC0 | (codepoint >> 6));
        text[1] = static_cast<char>(0x80 | (codepoint & 0x3F));
    } else if (codepoint <= 0xFFFF) {
        text[0] = static_cast<char>(0xE0 | (codepoint >> 12));
        text[1] = static_cast<char>(0x80 | ((codepoint >> 6) & 0x3F));
        text[2] = static_cast<char>(0x80 | (codepoint & 0x3F));
    } else {
        text[0] = static_cast<char>(0xF0 | (codepoint >> 18));
        text[1] = static_cast<char>(0x80 | ((codepoint >> 12) & 0x3F));
        text[2] = static_cast<char>(0x80 | ((codepoint >> 6) & 0x3F));
        text[3] = static_cast<char>(0x80 | (codepoint & 0x3F));
    }
    SDL_PushEvent(&event);
}

bool button(retro_input_state_t state, unsigned device, unsigned id) {
    return state && state(0, device, 0, id) != 0;
}

void push_mouse_motion(int x, int y, unsigned width, unsigned height) {
    SDL_Event event{};
    event.type = SDL_MOUSEMOTION;
    event.motion.x = x;
    event.motion.y = y;
    event.motion.xrel = 0;
    event.motion.yrel = 0;
    event.motion.windowID = 0;
    event.motion.state = SDL_GetMouseState(nullptr, nullptr);
    (void)width;
    (void)height;
    SDL_PushEvent(&event);
}

void push_mouse_button(bool down, Uint8 button_id, int x, int y) {
    SDL_Event event{};
    event.type = down ? SDL_MOUSEBUTTONDOWN : SDL_MOUSEBUTTONUP;
    event.button.button = button_id;
    event.button.state = down ? SDL_PRESSED : SDL_RELEASED;
    event.button.clicks = 1;
    event.button.x = x;
    event.button.y = y;
    SDL_PushEvent(&event);
}

} // namespace

SdlInputAdapter::SdlInputAdapter(retro_input_poll_t poll,
                                 retro_input_state_t state,
                                 unsigned width,
                                 unsigned height)
    : poll_(poll), state_(state), width_(std::max(1u, width)), height_(std::max(1u, height)),
      cursor_x_(static_cast<int>(width_ / 2)), cursor_y_(static_cast<int>(height_ / 2)) {}

void SdlInputAdapter::set_dimensions(unsigned width, unsigned height) {
    width_ = std::max(1u, width);
    height_ = std::max(1u, height);
    cursor_x_ = std::clamp(cursor_x_, 0, static_cast<int>(width_ - 1));
    cursor_y_ = std::clamp(cursor_y_, 0, static_cast<int>(height_ - 1));
}

void SdlInputAdapter::set_callbacks(retro_input_poll_t poll, retro_input_state_t state) {
    poll_ = poll;
    state_ = state;
}

void SdlInputAdapter::set_keyboard_callback_enabled(bool enabled) {
    keyboard_callback_enabled_ = enabled;
}

void SdlInputAdapter::keyboard_event(bool down,
                                    unsigned keycode,
                                    std::uint32_t character,
                                    std::uint16_t modifiers) {
    const SDL_Keymod sdl_modifiers = modifiers_for_retro(modifiers);
    SDL_SetModState(sdl_modifiers);

    const SDL_Keycode symbol = keycode_for_retro(keycode);
    if (symbol != SDLK_UNKNOWN) {
        SDL_Event event{};
        event.type = down ? SDL_KEYDOWN : SDL_KEYUP;
        event.key.state = down ? SDL_PRESSED : SDL_RELEASED;
        event.key.repeat = 0;
        event.key.keysym.sym = symbol;
        event.key.keysym.scancode = SDL_GetScancodeFromKey(symbol);
        event.key.keysym.mod = sdl_modifiers;
        SDL_PushEvent(&event);
    }

    if (down)
        push_text_input(character);
}

void SdlInputAdapter::reset() {
    key_down_.fill(false);
    left_down_ = false;
    right_down_ = false;
    middle_down_ = false;
}

void SdlInputAdapter::poll_and_enqueue() {
    if (!state_) {
        return;
    }
    if (poll_) {
        poll_();
    }

    if (!keyboard_callback_enabled_) {
        for (unsigned key = 0; key < RETROK_LAST; ++key) {
            const bool down = state_(0, RETRO_DEVICE_KEYBOARD, 0, key) != 0;
            if (down == key_down_[key]) {
                continue;
            }
            key_down_[key] = down;
            const SDL_Keycode symbol = keycode_for_retro(key);
            if (symbol == SDLK_UNKNOWN) {
                continue;
            }
            SDL_Event event{};
            event.type = down ? SDL_KEYDOWN : SDL_KEYUP;
            event.key.state = down ? SDL_PRESSED : SDL_RELEASED;
            event.key.repeat = 0;
            event.key.keysym.sym = symbol;
            event.key.keysym.scancode = SDL_GetScancodeFromKey(symbol);
            event.key.keysym.mod = KMOD_NONE;
            SDL_PushEvent(&event);
        }

        SDL_Keymod modifiers = KMOD_NONE;
        if (key_down_[RETROK_LSHIFT] || key_down_[RETROK_RSHIFT]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_SHIFT);
        if (key_down_[RETROK_LCTRL] || key_down_[RETROK_RCTRL]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_CTRL);
        if (key_down_[RETROK_LALT] || key_down_[RETROK_RALT]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_ALT);
        if (key_down_[RETROK_LMETA] || key_down_[RETROK_RMETA] || key_down_[RETROK_LSUPER] || key_down_[RETROK_RSUPER]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_GUI);
        if (key_down_[RETROK_CAPSLOCK]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_CAPS);
        if (key_down_[RETROK_NUMLOCK]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_NUM);
        SDL_SetModState(modifiers);
    }

    bool pointer_valid = false;
    const std::int16_t pointer_x = state_(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_X);
    const std::int16_t pointer_y = state_(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_Y);
    const bool pointer_pressed = state_(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_PRESSED) != 0;
    if (pointer_x || pointer_y || pointer_pressed) {
        cursor_x_ = static_cast<int>((static_cast<std::int32_t>(pointer_x) + 32768) * (width_ - 1) / 65535);
        cursor_y_ = static_cast<int>((static_cast<std::int32_t>(pointer_y) + 32768) * (height_ - 1) / 65535);
        pointer_valid = true;
    }

    if (!pointer_valid) {
        const int mouse_dx = state_(0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_X);
        const int mouse_dy = state_(0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_Y);
        const int analog_x = state_(0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_ID_ANALOG_X);
        const int analog_y = state_(0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_ID_ANALOG_Y);
        const int dpad_x = static_cast<int>(button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_RIGHT)) -
                           static_cast<int>(button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_LEFT));
        const int dpad_y = static_cast<int>(button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_DOWN)) -
                           static_cast<int>(button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_UP));
        cursor_x_ += mouse_dx + (std::abs(analog_x) > 4096 ? analog_x / 4096 : 0) + dpad_x * 8;
        cursor_y_ += mouse_dy + (std::abs(analog_y) > 4096 ? analog_y / 4096 : 0) + dpad_y * 8;
        cursor_x_ = std::clamp(cursor_x_, 0, static_cast<int>(width_ - 1));
        cursor_y_ = std::clamp(cursor_y_, 0, static_cast<int>(height_ - 1));
    }
    push_mouse_motion(cursor_x_, cursor_y_, width_, height_);

    const bool left = pointer_pressed || button(state_, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_LEFT) ||
                      button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_A);
    const bool right = button(state_, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_RIGHT) ||
                       button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_B);
    const bool middle = button(state_, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_MIDDLE);
    if (left != left_down_) { push_mouse_button(left, SDL_BUTTON_LEFT, cursor_x_, cursor_y_); left_down_ = left; }
    if (right != right_down_) { push_mouse_button(right, SDL_BUTTON_RIGHT, cursor_x_, cursor_y_); right_down_ = right; }
    if (middle != middle_down_) { push_mouse_button(middle, SDL_BUTTON_MIDDLE, cursor_x_, cursor_y_); middle_down_ = middle; }

    for (const auto id : {RETRO_DEVICE_ID_MOUSE_WHEELUP, RETRO_DEVICE_ID_MOUSE_WHEELDOWN}) {
        if (button(state_, RETRO_DEVICE_MOUSE, id)) {
            SDL_Event event{};
            event.type = SDL_MOUSEWHEEL;
            event.wheel.y = id == RETRO_DEVICE_ID_MOUSE_WHEELUP ? 1 : -1;
            SDL_PushEvent(&event);
        }
    }
}

} // namespace winebox
