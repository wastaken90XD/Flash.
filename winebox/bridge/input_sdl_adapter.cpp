#include "input_sdl_adapter.hpp"

#include <SDL.h>

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdlib>
#include <cstdio>
#include <fstream>
#include <string>
#include <utility>
#include <vector>

extern "C" {
void SDL_WineboxJoystickSetEnabled(SDL_bool enabled);
void SDL_WineboxJoystickSetAxis(int axis, Sint16 value);
void SDL_WineboxJoystickSetButton(int button, Uint8 value);
void SDL_WineboxJoystickSetHat(Uint8 value);
int winebox_boxedwine_queue_input_event(const SDL_Event* event);
}

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

void push_host_input_event(SDL_Event event) {
    if (!winebox_boxedwine_queue_input_event(&event))
        SDL_PushEvent(&event);
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
    push_host_input_event(event);
}

bool button(retro_input_state_t state, unsigned device, unsigned id) {
    return state && state(0, device, 0, id) != 0;
}

void push_key_event(bool down, unsigned key) {
    const SDL_Keycode symbol = keycode_for_retro(key);
    if (symbol == SDLK_UNKNOWN)
        return;
    SDL_Event event{};
    event.type = down ? SDL_KEYDOWN : SDL_KEYUP;
    event.key.state = down ? SDL_PRESSED : SDL_RELEASED;
    event.key.repeat = 0;
    event.key.keysym.sym = symbol;
    event.key.keysym.scancode = SDL_GetScancodeFromKey(symbol);
    event.key.keysym.mod = SDL_GetModState();
    push_host_input_event(event);
}

using OskKey = std::pair<const char*, unsigned>;

enum : unsigned {
    kActionMouseLeft = RETROK_LAST + 1,
    kActionMouseRight,
    kActionMouseMiddle,
    kActionMouseWheelUp,
    kActionMouseWheelDown,
    kActionMouseMoveUp,
    kActionMouseMoveDown,
    kActionMouseMoveLeft,
    kActionMouseMoveRight,
    kActionMouseSpeedUp,
    kActionMouseSpeedDown,
    kActionJoy1AxisLeft,
    kActionJoy1AxisRight,
    kActionJoy1AxisUp,
    kActionJoy1AxisDown,
    kActionJoy2AxisLeft,
    kActionJoy2AxisRight,
    kActionJoy2AxisUp,
    kActionJoy2AxisDown,
    kActionJoyButton1,
    kActionJoyButton2,
    kActionJoyButton3,
    kActionJoyButton4,
    kActionJoyButton5,
    kActionJoyButton6,
    kActionJoyButton7,
    kActionJoyButton8,
    kActionJoyHatUp,
    kActionJoyHatDown,
    kActionJoyHatLeft,
    kActionJoyHatRight,
};

const std::vector<OskKey>& osk_keys(bool include_actions = false) {
    static const std::vector<OskKey> keys = {{"NONE", 0}, {"A", RETROK_a}, {"B", RETROK_b}, {"C", RETROK_c},
        {"D", RETROK_d}, {"E", RETROK_e}, {"F", RETROK_f}, {"G", RETROK_g}, {"H", RETROK_h}, {"I", RETROK_i},
        {"J", RETROK_j}, {"K", RETROK_k}, {"L", RETROK_l}, {"M", RETROK_m}, {"N", RETROK_n}, {"O", RETROK_o},
        {"P", RETROK_p}, {"Q", RETROK_q}, {"R", RETROK_r}, {"S", RETROK_s}, {"T", RETROK_t}, {"U", RETROK_u},
        {"V", RETROK_v}, {"W", RETROK_w}, {"X", RETROK_x}, {"Y", RETROK_y}, {"Z", RETROK_z},
        {"0", RETROK_0}, {"1", RETROK_1}, {"2", RETROK_2}, {"3", RETROK_3}, {"4", RETROK_4},
        {"5", RETROK_5}, {"6", RETROK_6}, {"7", RETROK_7}, {"8", RETROK_8}, {"9", RETROK_9},
        {"SPACE", RETROK_SPACE}, {"ENTER", RETROK_RETURN}, {"BACK", RETROK_BACKSPACE}, {"ESC", RETROK_ESCAPE},
        {"SHIFT", RETROK_LSHIFT}, {"CTRL", RETROK_LCTRL}, {"ALT", RETROK_LALT}, {"TAB", RETROK_TAB},
        {"UP", RETROK_UP}, {"DOWN", RETROK_DOWN}, {"LEFT", RETROK_LEFT}, {"RIGHT", RETROK_RIGHT},
        {"HOME", RETROK_HOME}, {"END", RETROK_END}, {"PGUP", RETROK_PAGEUP}, {"PGDN", RETROK_PAGEDOWN},
        {"INS", RETROK_INSERT}, {"DEL", RETROK_DELETE}, {"F1", RETROK_F1}, {"F2", RETROK_F2},
        {"F3", RETROK_F3}, {"F4", RETROK_F4}, {"F5", RETROK_F5}, {"F6", RETROK_F6},
        {"F7", RETROK_F7}, {"F8", RETROK_F8}, {"F9", RETROK_F9}, {"F10", RETROK_F10},
        {"F11", RETROK_F11}, {"F12", RETROK_F12}, {"MINUS", RETROK_MINUS}, {"EQUAL", RETROK_EQUALS},
        {"LBRACK", RETROK_LEFTBRACKET}, {"RBRACK", RETROK_RIGHTBRACKET}, {"COMMA", RETROK_COMMA},
        {"PERIOD", RETROK_PERIOD}, {"SLASH", RETROK_SLASH}, {"BACKSLASH", RETROK_BACKSLASH},
        {"GRAVE", RETROK_BACKQUOTE}, {"SEMICOLON", RETROK_SEMICOLON}, {"QUOTE", RETROK_QUOTE},
        {"CAPS", RETROK_CAPSLOCK}, {"NUM", RETROK_NUMLOCK}, {"SCROLL", RETROK_SCROLLOCK},
        {"PRINT", RETROK_PRINT}, {"PAUSE", RETROK_PAUSE}, {"RSHIFT", RETROK_RSHIFT},
        {"RCTRL", RETROK_RCTRL}, {"RALT", RETROK_RALT}, {"KP0", RETROK_KP0}, {"KP1", RETROK_KP1},
        {"KP2", RETROK_KP2}, {"KP3", RETROK_KP3}, {"KP4", RETROK_KP4}, {"KP5", RETROK_KP5},
        {"KP6", RETROK_KP6}, {"KP7", RETROK_KP7}, {"KP8", RETROK_KP8}, {"KP9", RETROK_KP9},
        {"KP PERIOD", RETROK_KP_PERIOD}, {"KP DIVIDE", RETROK_KP_DIVIDE}, {"KP MULTIPLY", RETROK_KP_MULTIPLY},
        {"KP MINUS", RETROK_KP_MINUS}, {"KP PLUS", RETROK_KP_PLUS}, {"KP ENTER", RETROK_KP_ENTER},
        {"INSERT", RETROK_INSERT}, {"DELETE", RETROK_DELETE}, {"SUPER", RETROK_LSUPER}};
    static const std::vector<OskKey> full_options = [] {
        auto result = keys;
        result.insert(result.end(), {{"MOUSE LEFT", kActionMouseLeft}, {"MOUSE RIGHT", kActionMouseRight},
            {"MOUSE MIDDLE", kActionMouseMiddle}, {"WHEEL UP", kActionMouseWheelUp},
            {"WHEEL DOWN", kActionMouseWheelDown}, {"MOVE UP", kActionMouseMoveUp},
            {"MOVE DOWN", kActionMouseMoveDown}, {"MOVE LEFT", kActionMouseMoveLeft},
            {"MOVE RIGHT", kActionMouseMoveRight}, {"SPEED UP", kActionMouseSpeedUp},
            {"SLOW DOWN", kActionMouseSpeedDown}, {"JOY1 LEFT", kActionJoy1AxisLeft},
            {"JOY1 RIGHT", kActionJoy1AxisRight}, {"JOY1 UP", kActionJoy1AxisUp},
            {"JOY1 DOWN", kActionJoy1AxisDown}, {"JOY2 LEFT", kActionJoy2AxisLeft},
            {"JOY2 RIGHT", kActionJoy2AxisRight}, {"JOY2 UP", kActionJoy2AxisUp},
            {"JOY2 DOWN", kActionJoy2AxisDown}, {"JOY BUTTON1", kActionJoyButton1},
            {"JOY BUTTON2", kActionJoyButton2}, {"JOY BUTTON3", kActionJoyButton3},
            {"JOY BUTTON4", kActionJoyButton4}, {"JOY BUTTON5", kActionJoyButton5},
            {"JOY BUTTON6", kActionJoyButton6}, {"JOY BUTTON7", kActionJoyButton7},
            {"JOY BUTTON8", kActionJoyButton8}, {"JOY HAT UP", kActionJoyHatUp},
            {"JOY HAT DOWN", kActionJoyHatDown}, {"JOY HAT LEFT", kActionJoyHatLeft},
            {"JOY HAT RIGHT", kActionJoyHatRight}});
        return result;
    }();
    return include_actions ? full_options : keys;
}

std::array<std::uint8_t, 5> glyph(char c) {
    switch (c) {
    case 'A': return {7,5,7,5,5}; case 'B': return {6,5,6,5,6}; case 'C': return {7,4,4,4,7};
    case 'D': return {6,5,5,5,6}; case 'E': return {7,4,6,4,7}; case 'F': return {7,4,6,4,4};
    case 'G': return {7,4,5,5,7}; case 'H': return {5,5,7,5,5}; case 'I': return {7,2,2,2,7};
    case 'J': return {3,1,1,5,7}; case 'K': return {5,5,6,5,5}; case 'L': return {4,4,4,4,7};
    case 'M': return {5,7,7,5,5}; case 'N': return {5,7,7,7,5}; case 'O': return {7,5,5,5,7};
    case 'P': return {7,5,7,4,4}; case 'Q': return {7,5,5,7,1}; case 'R': return {6,5,6,5,5};
    case 'S': return {7,4,7,1,7}; case 'T': return {7,2,2,2,2}; case 'U': return {5,5,5,5,7};
    case 'V': return {5,5,5,5,2}; case 'W': return {5,5,7,7,5}; case 'X': return {5,5,2,5,5};
    case 'Y': return {5,5,2,2,2}; case 'Z': return {7,1,2,4,7};
    case '0': return {7,5,5,5,7}; case '1': return {2,6,2,2,7}; case '2': return {6,1,7,4,7};
    case '3': return {6,1,6,1,6}; case '4': return {5,5,7,1,1}; case '5': return {7,4,6,1,6};
    case '6': return {3,4,7,5,7}; case '7': return {7,1,2,2,2}; case '8': return {7,5,7,5,7};
    case '9': return {7,5,7,1,6}; case ':': return {0,2,0,2,0}; case '-': return {0,0,7,0,0};
    case '/': return {1,1,2,4,4}; case '.': return {0,0,0,0,2}; case '>': return {4,2,1,2,4};
    default: return {0,0,0,0,0};
    }
}

void draw_rect(std::uint32_t* pixels, unsigned width, unsigned height, int x, int y, int w, int h, std::uint32_t color) {
    const int x0 = std::clamp(x, 0, static_cast<int>(width));
    const int y0 = std::clamp(y, 0, static_cast<int>(height));
    const int x1 = std::clamp(x + w, 0, static_cast<int>(width));
    const int y1 = std::clamp(y + h, 0, static_cast<int>(height));
    for (int yy = y0; yy < y1; ++yy)
        std::fill(pixels + static_cast<std::size_t>(yy) * width + x0,
                  pixels + static_cast<std::size_t>(yy) * width + x1, color);
}

std::string action_name(unsigned target) {
    if (target == 0)
        return "NONE";
    if (target >= RETROK_LAST) {
        switch (target) {
        case kActionMouseLeft: return "MOUSE LEFT";
        case kActionMouseRight: return "MOUSE RIGHT";
        case kActionMouseMiddle: return "MOUSE MIDDLE";
        case kActionMouseWheelUp: return "WHEEL UP";
        case kActionMouseWheelDown: return "WHEEL DOWN";
        case kActionMouseMoveUp: return "MOVE UP";
        case kActionMouseMoveDown: return "MOVE DOWN";
        case kActionMouseMoveLeft: return "MOVE LEFT";
        case kActionMouseMoveRight: return "MOVE RIGHT";
        case kActionMouseSpeedUp: return "SPEED UP";
        case kActionMouseSpeedDown: return "SLOW DOWN";
        case kActionJoy1AxisLeft: return "JOY1 LEFT";
        case kActionJoy1AxisRight: return "JOY1 RIGHT";
        case kActionJoy1AxisUp: return "JOY1 UP";
        case kActionJoy1AxisDown: return "JOY1 DOWN";
        case kActionJoy2AxisLeft: return "JOY2 LEFT";
        case kActionJoy2AxisRight: return "JOY2 RIGHT";
        case kActionJoy2AxisUp: return "JOY2 UP";
        case kActionJoy2AxisDown: return "JOY2 DOWN";
        case kActionJoyButton1: return "JOY BUTTON1";
        case kActionJoyButton2: return "JOY BUTTON2";
        case kActionJoyButton3: return "JOY BUTTON3";
        case kActionJoyButton4: return "JOY BUTTON4";
        case kActionJoyButton5: return "JOY BUTTON5";
        case kActionJoyButton6: return "JOY BUTTON6";
        case kActionJoyButton7: return "JOY BUTTON7";
        case kActionJoyButton8: return "JOY BUTTON8";
        case kActionJoyHatUp: return "JOY HAT UP";
        case kActionJoyHatDown: return "JOY HAT DOWN";
        case kActionJoyHatLeft: return "JOY HAT LEFT";
        case kActionJoyHatRight: return "JOY HAT RIGHT";
        default: return "UNKNOWN";
        }
    }
    for (const auto& entry : osk_keys())
        if (entry.second == target)
            return entry.first;
    if (target == RETROK_UP) return "UP";
    if (target == RETROK_DOWN) return "DOWN";
    if (target == RETROK_LEFT) return "LEFT";
    if (target == RETROK_RIGHT) return "RIGHT";
    if (target == RETROK_HOME) return "HOME";
    if (target == RETROK_END) return "END";
    if (target == RETROK_PAGEUP) return "PAGE UP";
    if (target == RETROK_PAGEDOWN) return "PAGE DOWN";
    if (target >= RETROK_F1 && target <= RETROK_F12)
        return "F" + std::to_string(target - RETROK_F1 + 1);
    return "KEY";
}

void draw_text(std::uint32_t* pixels, unsigned width, unsigned height, int x, int y,
               const std::string& text, std::uint32_t color, int scale = 1) {
    for (char raw : text) {
        const char c = (raw >= 'a' && raw <= 'z') ? static_cast<char>(raw - 'a' + 'A') : raw;
        const auto bits = glyph(c);
        for (int row = 0; row < 5; ++row)
            for (int col = 0; col < 3; ++col)
                if (bits[row] & (1u << (2 - col)))
                    draw_rect(pixels, width, height, x + col * scale, y + row * scale, scale, scale, color);
        x += 4 * scale;
    }
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
    push_host_input_event(event);
}

void push_mouse_button(bool down, Uint8 button_id, int x, int y) {
    SDL_Event event{};
    event.type = down ? SDL_MOUSEBUTTONDOWN : SDL_MOUSEBUTTONUP;
    event.button.button = button_id;
    event.button.state = down ? SDL_PRESSED : SDL_RELEASED;
    event.button.clicks = 1;
    event.button.x = x;
    event.button.y = y;
    push_host_input_event(event);
}

} // namespace

SdlInputAdapter::SdlInputAdapter(retro_input_poll_t poll,
                                 retro_input_state_t state,
                                 unsigned width,
                                 unsigned height)
    : poll_(poll), state_(state), width_(std::max(1u, width)), height_(std::max(1u, height)),
      cursor_x_(static_cast<int>(width_ / 2)), cursor_y_(static_cast<int>(height_ / 2)) {
    load_default_mapping();
}

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

void SdlInputAdapter::set_controller_mode(ControllerMode mode) {
    if (controller_mode_ == mode)
        return;
    controller_mode_ = mode;
    mapped_key_down_.fill(false);
    mapped_mouse_down_.fill(false);
    sync_all_key_states();
}

void SdlInputAdapter::load_default_mapping() {
    for (auto& actions : mapping_)
        actions.fill(0);
    const std::array<unsigned, 24> defaults = {RETROK_LCTRL, RETROK_LSHIFT, RETROK_ESCAPE, RETROK_RETURN,
        RETROK_w, RETROK_s, RETROK_a, RETROK_d, RETROK_LALT, RETROK_SPACE,
        RETROK_1, RETROK_2, RETROK_3, RETROK_4, 0, RETROK_F2,
        RETROK_a, RETROK_d, RETROK_w, RETROK_s,
        RETROK_LEFT, RETROK_RIGHT, RETROK_UP, RETROK_DOWN};
    for (std::size_t source = 0; source < defaults.size(); ++source)
        mapping_[source][0] = defaults[source];
}

void SdlInputAdapter::set_config_file(std::string path) {
    config_file_ = std::move(path);
    load_default_mapping();
    load_mapping();
    sync_virtual_joystick();
}

void SdlInputAdapter::sync_virtual_joystick() {
    bool needed = false;
    for (const auto& actions : mapping_) {
        for (unsigned action : actions) {
            if (action >= kActionJoy1AxisLeft && action <= kActionJoyHatRight) {
                needed = true;
                break;
            }
        }
        if (needed)
            break;
    }
    SDL_WineboxJoystickSetEnabled(needed ? SDL_TRUE : SDL_FALSE);
}

void SdlInputAdapter::load_mapping() {
    if (config_file_.empty())
        return;
    std::ifstream input(config_file_);
    std::string signature;
    unsigned version = 0;
    if (!(input >> signature >> version) || signature != "winebox-controller-map" || (version != 1 && version != 2))
        return;
    auto loaded = mapping_;
    if (version == 1) {
        for (auto& actions : loaded) {
            unsigned key = 0;
            if (!(input >> key) || key >= RETROK_LAST)
                return;
            actions.fill(0);
            actions[0] = key;
        }
    } else {
        for (auto& actions : loaded) {
            for (unsigned& action : actions) {
                if (!(input >> action) || (action >= RETROK_LAST && action > kActionJoyHatRight))
                    return;
            }
        }
    }
    std::string extra;
    if (input >> extra)
        return;
    mapping_ = loaded;
}

void SdlInputAdapter::save_mapping() const {
    if (config_file_.empty())
        return;
    const std::string temporary = config_file_ + ".tmp";
    {
        std::ofstream output(temporary, std::ios::trunc);
        if (!output)
            return;
        output << "winebox-controller-map 2\n";
        for (const auto& actions : mapping_)
            for (unsigned action : actions)
                output << action << '\n';
        if (!output)
            return;
    }
    std::remove(config_file_.c_str());
    std::rename(temporary.c_str(), config_file_.c_str());
}

void SdlInputAdapter::update_sdl_modifiers() {
    auto active = [this](unsigned key) {
        return key_down_[key] || callback_key_down_[key] || mapped_key_down_[key] || osk_latched_modifier_ == key;
    };
    SDL_Keymod modifiers = KMOD_NONE;
    if (active(RETROK_LSHIFT) || active(RETROK_RSHIFT)) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_SHIFT);
    if (active(RETROK_LCTRL) || active(RETROK_RCTRL)) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_CTRL);
    if (active(RETROK_LALT) || active(RETROK_RALT)) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_ALT);
    if (active(RETROK_LMETA) || active(RETROK_RMETA) || active(RETROK_LSUPER) || active(RETROK_RSUPER))
        modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_GUI);
    if (active(RETROK_CAPSLOCK)) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_CAPS);
    if (active(RETROK_NUMLOCK)) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_NUM);
    SDL_SetModState(modifiers);
}

void SdlInputAdapter::sync_key_state(unsigned key) {
    update_sdl_modifiers();
    if (key >= output_key_down_.size())
        return;
    const bool down = key_down_[key] || callback_key_down_[key] || mapped_key_down_[key];
    if (down != output_key_down_[key]) {
        push_key_event(down, key);
        output_key_down_[key] = down;
    }
}

void SdlInputAdapter::sync_all_key_states() {
    for (unsigned key = 0; key < output_key_down_.size(); ++key)
        sync_key_state(key);
}

void SdlInputAdapter::handle_overlay_input(const std::array<bool, 16>& buttons,
                                           const std::array<std::int16_t, 4>& axes) {
    const bool was_closed = overlay_mode_ == OverlayMode::Closed;
    const bool was_open = !was_closed;
    std::array<bool, 16> pressed{};
    for (std::size_t i = 0; i < buttons.size(); ++i)
        pressed[i] = buttons[i] && !ui_button_previous_[i];
    const std::array<bool, 4> axis_active = {axes[0] < -12000, axes[0] > 12000,
                                             axes[1] < -12000, axes[1] > 12000};
    const bool nav_left = pressed[RETRO_DEVICE_ID_JOYPAD_LEFT] || (axis_active[0] && !ui_axis_previous_[0]);
    const bool nav_right = pressed[RETRO_DEVICE_ID_JOYPAD_RIGHT] || (axis_active[1] && !ui_axis_previous_[1]);
    const bool nav_up = pressed[RETRO_DEVICE_ID_JOYPAD_UP] || (axis_active[2] && !ui_axis_previous_[2]);
    const bool nav_down = pressed[RETRO_DEVICE_ID_JOYPAD_DOWN] || (axis_active[3] && !ui_axis_previous_[3]);
    const bool accept = pressed[RETRO_DEVICE_ID_JOYPAD_A] || pressed[RETRO_DEVICE_ID_JOYPAD_START];
    const bool cancel = pressed[RETRO_DEVICE_ID_JOYPAD_B] || pressed[RETRO_DEVICE_ID_JOYPAD_SELECT];

    if (overlay_mode_ == OverlayMode::Closed) {
        if (pressed[RETRO_DEVICE_ID_JOYPAD_L3]) {
            overlay_mode_ = OverlayMode::Menu;
            menu_selection_ = 0;
        }
    } else if (overlay_mode_ == OverlayMode::Menu) {
        if (nav_up) menu_selection_ = (menu_selection_ + 2) % 3;
        if (nav_down) menu_selection_ = (menu_selection_ + 1) % 3;
        if (cancel) overlay_mode_ = OverlayMode::Closed;
        if (accept) {
            if (menu_selection_ == 0) overlay_mode_ = OverlayMode::Mapper;
            else if (menu_selection_ == 1) overlay_mode_ = OverlayMode::Keyboard;
            else overlay_mode_ = OverlayMode::Closed;
        }
    } else if (overlay_mode_ == OverlayMode::Mapper) {
        if (nav_up) selected_control_ = (selected_control_ + 23) % 24;
        if (nav_down) selected_control_ = (selected_control_ + 1) % 24;
        if (cancel) overlay_mode_ = OverlayMode::Menu;
        if (accept && selected_control_ != RETRO_DEVICE_ID_JOYPAD_L3)
            overlay_mode_ = OverlayMode::MapperSlots;
    } else if (overlay_mode_ == OverlayMode::MapperSlots) {
        if (nav_up) selected_action_slot_ = (selected_action_slot_ + 3) % 4;
        if (nav_down) selected_action_slot_ = (selected_action_slot_ + 1) % 4;
        if (cancel) overlay_mode_ = OverlayMode::Mapper;
        if (accept) {
            overlay_mode_ = OverlayMode::AssignKeyboard;
            keyboard_selection_ = 0;
        }
    } else {
        const bool assigning = overlay_mode_ == OverlayMode::AssignKeyboard;
        const auto& options = osk_keys(assigning);
        const unsigned key_count = static_cast<unsigned>(options.size());
        if (nav_up) keyboard_selection_ = (keyboard_selection_ + key_count - 8) % key_count;
        if (nav_down) keyboard_selection_ = (keyboard_selection_ + 8) % key_count;
        if (nav_left) keyboard_selection_ = (keyboard_selection_ + key_count - 1) % key_count;
        if (nav_right) keyboard_selection_ = (keyboard_selection_ + 1) % key_count;
        if (cancel) {
            if (osk_latched_modifier_) {
                const unsigned modifier = osk_latched_modifier_;
                osk_latched_modifier_ = 0;
                update_sdl_modifiers();
                push_key_event(false, modifier);
            }
            overlay_mode_ = assigning ? OverlayMode::MapperSlots : OverlayMode::Menu;
        }
        if (accept) {
            const unsigned key = options[keyboard_selection_].second;
            if (assigning) {
                mapping_[selected_control_][selected_action_slot_] = key;
                save_mapping();
                sync_virtual_joystick();
                overlay_mode_ = OverlayMode::MapperSlots;
            } else if (key < RETROK_LAST && (key == RETROK_LSHIFT || key == RETROK_LCTRL || key == RETROK_LALT)) {
                if (osk_latched_modifier_ == key) {
                    osk_latched_modifier_ = 0;
                    update_sdl_modifiers();
                    push_key_event(false, key);
                } else {
                    if (osk_latched_modifier_) {
                        const unsigned old_modifier = osk_latched_modifier_;
                        osk_latched_modifier_ = 0;
                        update_sdl_modifiers();
                        push_key_event(false, old_modifier);
                    }
                    osk_latched_modifier_ = key;
                    update_sdl_modifiers();
                    push_key_event(true, key);
                }
            } else if (key) {
                push_key_event(true, key);
                push_key_event(false, key);
                if (osk_latched_modifier_) {
                    const unsigned modifier = osk_latched_modifier_;
                    osk_latched_modifier_ = 0;
                    update_sdl_modifiers();
                    push_key_event(false, modifier);
                }
            }
        }
    }
    if (was_open && overlay_mode_ == OverlayMode::Closed) {
        for (std::size_t source = 0; source < buttons.size(); ++source)
            release_gate_[source] = buttons[source];
        for (std::size_t axis = 0; axis < axes.size(); ++axis) {
            release_gate_[16 + axis * 2] = axes[axis] <= -12000;
            release_gate_[17 + axis * 2] = axes[axis] >= 12000;
        }
    }
    if (was_closed && overlay_mode_ != OverlayMode::Closed) {
        for (std::size_t key = 0; key < key_down_.size(); ++key) {
            key_down_[key] = false;
            callback_key_down_[key] = false;
        }
    }
    ui_button_previous_ = buttons;
    ui_axis_previous_ = axis_active;
}

bool SdlInputAdapter::overlay_active() const noexcept {
    return overlay_mode_ != OverlayMode::Closed;
}

void SdlInputAdapter::render_overlay(std::uint32_t* pixels, unsigned width, unsigned height) const {
    if (!pixels || !width || !height || overlay_mode_ == OverlayMode::Closed)
        return;
    const int panel_w = std::min(static_cast<int>(width) - 16, 620);
    const int panel_h = std::min(static_cast<int>(height) - 16, overlay_mode_ == OverlayMode::Keyboard || overlay_mode_ == OverlayMode::AssignKeyboard ? 440 : 270);
    if (panel_w < 100 || panel_h < 100)
        return;
    const int left = (static_cast<int>(width) - panel_w) / 2;
    const int top = (static_cast<int>(height) - panel_h) / 2;
    draw_rect(pixels, width, height, left, top, panel_w, panel_h, 0x00131D2Au);
    draw_rect(pixels, width, height, left, top, panel_w, 3, 0x0000B8D4u);
    draw_rect(pixels, width, height, left, top + panel_h - 3, panel_w, 3, 0x0000B8D4u);
    draw_rect(pixels, width, height, left, top, 3, panel_h, 0x0000B8D4u);
    draw_rect(pixels, width, height, left + panel_w - 3, top, 3, panel_h, 0x0000B8D4u);

    std::string title = "WINEBOX CONTROLS";
    if (overlay_mode_ == OverlayMode::Mapper) title = "CONTROLLER MAPPER";
    else if (overlay_mode_ == OverlayMode::MapperSlots) title = "BINDING ACTIONS";
    else if (overlay_mode_ == OverlayMode::Keyboard) title = "ON SCREEN KEYBOARD";
    else if (overlay_mode_ == OverlayMode::AssignKeyboard) title = "ASSIGN ACTION TO CONTROL";
    draw_text(pixels, width, height, left + 18, top + 14, title, 0x00FFFFFFu, 2);

    if (overlay_mode_ == OverlayMode::Menu) {
        const std::array<const char*, 3> labels = {"CONTROLLER MAPPER", "ON SCREEN KEYBOARD", "CLOSE MENU"};
        for (unsigned i = 0; i < labels.size(); ++i) {
            const int y = top + 66 + static_cast<int>(i) * 46;
            draw_rect(pixels, width, height, left + 16, y - 5, panel_w - 32, 34,
                      i == menu_selection_ ? 0x00304458u : 0x001B2835u);
            draw_text(pixels, width, height, left + 28, y + 3, labels[i], 0x00FFFFFFu, 2);
        }
        draw_text(pixels, width, height, left + 18, top + panel_h - 26, "UP DOWN SELECT BACK", 0x009FB3C8u, 1);
        return;
    }

    if (overlay_mode_ == OverlayMode::Mapper) {
        const std::array<const char*, 16> buttons = {"B", "Y", "SELECT", "START", "DPAD UP", "DPAD DOWN", "DPAD LEFT", "DPAD RIGHT",
            "A", "X", "L", "R", "L2", "R2", "L3 MENU", "R3"};
        const unsigned page = selected_control_ / 8;
        const unsigned first = page * 8;
        for (unsigned i = first; i < std::min(first + 8, 24u); ++i) {
            const int y = top + 48 + static_cast<int>(i - first) * 22;
            std::string source;
            if (i < 16) source = buttons[i];
            else {
                static const std::array<const char*, 8> axes = {"LEFT X LEFT", "LEFT X RIGHT", "LEFT Y UP", "LEFT Y DOWN",
                    "RIGHT X LEFT", "RIGHT X RIGHT", "RIGHT Y UP", "RIGHT Y DOWN"};
                source = axes[i - 16];
            }
            std::string targets;
            for (unsigned action : mapping_[i]) {
                if (action == 0) continue;
                if (!targets.empty()) targets += " ";
                targets += action_name(action);
            }
            if (targets.empty()) targets = "NONE";
            draw_rect(pixels, width, height, left + 14, y - 3, panel_w - 28, 19,
                      i == selected_control_ ? 0x00304458u : 0x001B2835u);
            draw_text(pixels, width, height, left + 22, y, source, 0x00FFFFFFu, 1);
            draw_text(pixels, width, height, left + 160, y, targets, 0x0000E5FFu, 1);
        }
        std::string page_label = "PAGE " + std::to_string(page + 1) + " OF 3";
        draw_text(pixels, width, height, left + 18, top + panel_h - 28, page_label + "  A EDIT  B BACK", 0x009FB3C8u, 1);
        return;
    }

    if (overlay_mode_ == OverlayMode::MapperSlots) {
        static const std::array<const char*, 24> labels = {"B", "Y", "SELECT", "START", "DPAD UP", "DPAD DOWN", "DPAD LEFT", "DPAD RIGHT",
            "A", "X", "L", "R", "L2", "R2", "L3 MENU", "R3", "LEFT X LEFT", "LEFT X RIGHT", "LEFT Y UP", "LEFT Y DOWN",
            "RIGHT X LEFT", "RIGHT X RIGHT", "RIGHT Y UP", "RIGHT Y DOWN"};
        draw_text(pixels, width, height, left + 18, top + 42, "CONTROL " + std::string(labels[selected_control_]), 0x00B8D7FFu, 1);
        for (unsigned slot = 0; slot < 4; ++slot) {
            const int y = top + 72 + static_cast<int>(slot) * 38;
            draw_rect(pixels, width, height, left + 14, y - 5, panel_w - 28, 30,
                      slot == selected_action_slot_ ? 0x00304458u : 0x001B2835u);
            draw_text(pixels, width, height, left + 24, y + 2, "ACTION " + std::to_string(slot + 1), 0x00FFFFFFu, 1);
            draw_text(pixels, width, height, left + 210, y + 2,
                      action_name(mapping_[selected_control_][slot]), 0x0000E5FFu, 1);
        }
        draw_text(pixels, width, height, left + 18, top + panel_h - 24, "A EDIT  NONE CLEARS  B BACK", 0x009FB3C8u, 1);
        return;
    }

    const auto& keys = osk_keys(overlay_mode_ == OverlayMode::AssignKeyboard);
    const unsigned columns = 8;
    const int cell_w = (panel_w - 36) / static_cast<int>(columns);
    const int cell_h = 28;
    const unsigned total_rows = static_cast<unsigned>((keys.size() + columns - 1) / columns);
    const unsigned visible_rows = static_cast<unsigned>(std::max(1, (panel_h - 92) / cell_h));
    const unsigned selected_row = static_cast<unsigned>(keyboard_selection_ / columns);
    const unsigned max_first_row = total_rows > visible_rows ? total_rows - visible_rows : 0;
    const unsigned first_row = std::min(max_first_row,
        selected_row > visible_rows / 2 ? selected_row - visible_rows / 2 : 0);
    const unsigned end_row = std::min(total_rows, first_row + visible_rows);
    for (unsigned row = first_row; row < end_row; ++row) {
        for (unsigned col = 0; col < columns; ++col) {
            const unsigned i = row * columns + col;
            if (i >= keys.size())
                break;
            const int x = left + 12 + static_cast<int>(col) * cell_w;
            const int y = top + 52 + static_cast<int>(row - first_row) * cell_h;
            draw_rect(pixels, width, height, x, y, cell_w - 3, cell_h - 3,
                      i == keyboard_selection_ ? 0x0000A9C6u : 0x00233241u);
            draw_text(pixels, width, height, x + 3, y + 7, keys[i].first, 0x00FFFFFFu, 1);
        }
    }
    const char* footer = overlay_mode_ == OverlayMode::AssignKeyboard
        ? "PICK KEY OR MOUSE ACTION  NONE CLEARS"
        : "PICK KEY TO TYPE  B BACK";
    draw_text(pixels, width, height, left + 16, top + panel_h - 22, footer, 0x009FB3C8u, 1);
}

void SdlInputAdapter::keyboard_event(bool down,
                                    unsigned keycode,
                                    std::uint32_t character,
                                    std::uint16_t modifiers) {
    const bool input_allowed = overlay_mode_ == OverlayMode::Closed;
    if (input_allowed)
        SDL_SetModState(modifiers_for_retro(modifiers));
    if (keycode < callback_key_down_.size()) {
        callback_key_down_[keycode] = down && input_allowed;
        sync_key_state(keycode);
    }
    if (!input_allowed)
        return;
    if (down)
        push_text_input(character);
}

void SdlInputAdapter::reset() {
    for (std::size_t key = 0; key < output_key_down_.size(); ++key) {
        if (output_key_down_[key])
            push_key_event(false, static_cast<unsigned>(key));
    }
    if (osk_latched_modifier_)
        push_key_event(false, osk_latched_modifier_);
    if (left_down_) push_mouse_button(false, SDL_BUTTON_LEFT, cursor_x_, cursor_y_);
    if (right_down_) push_mouse_button(false, SDL_BUTTON_RIGHT, cursor_x_, cursor_y_);
    if (middle_down_) push_mouse_button(false, SDL_BUTTON_MIDDLE, cursor_x_, cursor_y_);
    key_down_.fill(false);
    callback_key_down_.fill(false);
    mapped_key_down_.fill(false);
    output_key_down_.fill(false);
    mapped_source_down_.fill(false);
    mapped_mouse_down_.fill(false);
    release_gate_.fill(false);
    ui_button_previous_.fill(false);
    ui_axis_previous_.fill(false);
    overlay_mode_ = OverlayMode::Closed;
    osk_latched_modifier_ = 0;
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
            key_down_[key] = down && overlay_mode_ == OverlayMode::Closed;
        }

        SDL_Keymod modifiers = KMOD_NONE;
        if (key_down_[RETROK_LSHIFT] || key_down_[RETROK_RSHIFT]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_SHIFT);
        if (key_down_[RETROK_LCTRL] || key_down_[RETROK_RCTRL]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_CTRL);
        if (key_down_[RETROK_LALT] || key_down_[RETROK_RALT]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_ALT);
        if (key_down_[RETROK_LMETA] || key_down_[RETROK_RMETA] || key_down_[RETROK_LSUPER] || key_down_[RETROK_RSUPER]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_GUI);
        if (key_down_[RETROK_CAPSLOCK]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_CAPS);
        if (key_down_[RETROK_NUMLOCK]) modifiers = static_cast<SDL_Keymod>(modifiers | KMOD_NUM);
        SDL_SetModState(modifiers);
        sync_all_key_states();
    }

    std::array<bool, 16> current_buttons{};
    for (unsigned id = 0; id < current_buttons.size(); ++id)
        current_buttons[id] = button(state_, RETRO_DEVICE_JOYPAD, id);
    const std::array<std::int16_t, 4> axes = {
        state_(0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_ID_ANALOG_X),
        state_(0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_ID_ANALOG_Y),
        state_(0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_RIGHT, RETRO_DEVICE_ID_ANALOG_X),
        state_(0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_RIGHT, RETRO_DEVICE_ID_ANALOG_Y)
    };
    handle_overlay_input(current_buttons, axes);

    std::array<bool, 24> next_mapped_sources{};
    const bool mapping_enabled = controller_mode_ == ControllerMode::GenericKeyboard ||
                                 controller_mode_ == ControllerMode::CustomMapper;
    if (mapping_enabled && overlay_mode_ == OverlayMode::Closed) {
        for (unsigned id = 0; id < 16; ++id)
            next_mapped_sources[id] = current_buttons[id];
        constexpr std::int16_t kAxisThreshold = 12000;
        for (std::size_t axis = 0; axis < axes.size(); ++axis) {
            next_mapped_sources[16 + axis * 2] = axes[axis] <= -kAxisThreshold;
            next_mapped_sources[17 + axis * 2] = axes[axis] >= kAxisThreshold;
        }
    }
    for (std::size_t source = 0; source < next_mapped_sources.size(); ++source) {
        if (release_gate_[source] && !next_mapped_sources[source])
            release_gate_[source] = false;
        if (release_gate_[source])
            next_mapped_sources[source] = false;
    }
    std::array<unsigned, RETROK_LAST> mapped_key_counts{};
    std::array<unsigned, 3> mapped_mouse_counts{};
    int mapped_cursor_dx = 0, mapped_cursor_dy = 0;
    bool mapped_wheel_up = false, mapped_wheel_down = false;
    bool mapped_speed_up = false, mapped_speed_down = false;
    std::array<int, 4> mapped_joy_axes{};
    std::array<unsigned, 8> mapped_joy_buttons{};
    Uint8 mapped_joy_hat = SDL_HAT_CENTERED;
    bool has_joystick_bindings = false;
    for (const auto& actions : mapping_)
        for (unsigned action : actions)
            has_joystick_bindings |= action >= kActionJoy1AxisLeft && action <= kActionJoyHatRight;
    for (std::size_t source = 0; source < next_mapped_sources.size(); ++source) {
        if (!next_mapped_sources[source])
            continue;
        const bool just_pressed = !mapped_source_down_[source];
        for (unsigned action : mapping_[source]) {
            if (action == 0)
                continue;
            if (action < RETROK_LAST) {
                ++mapped_key_counts[action];
            } else {
                switch (action) {
                case kActionMouseLeft: ++mapped_mouse_counts[0]; break;
                case kActionMouseRight: ++mapped_mouse_counts[1]; break;
                case kActionMouseMiddle: ++mapped_mouse_counts[2]; break;
                case kActionMouseWheelUp: mapped_wheel_up |= just_pressed; break;
                case kActionMouseWheelDown: mapped_wheel_down |= just_pressed; break;
                case kActionMouseMoveUp: --mapped_cursor_dy; break;
                case kActionMouseMoveDown: ++mapped_cursor_dy; break;
                case kActionMouseMoveLeft: --mapped_cursor_dx; break;
                case kActionMouseMoveRight: ++mapped_cursor_dx; break;
                case kActionMouseSpeedUp: mapped_speed_up = true; break;
                case kActionMouseSpeedDown: mapped_speed_down = true; break;
                case kActionJoy1AxisLeft: case kActionJoy1AxisRight:
                case kActionJoy1AxisUp: case kActionJoy1AxisDown:
                case kActionJoy2AxisLeft: case kActionJoy2AxisRight:
                case kActionJoy2AxisUp: case kActionJoy2AxisDown: {
                    const bool second_stick = action >= kActionJoy2AxisLeft;
                    const unsigned axis_action = second_stick
                        ? action - kActionJoy2AxisLeft : action - kActionJoy1AxisLeft;
                    const unsigned axis = (second_stick ? 2u : 0u) + axis_action / 2;
                    const bool negative = (axis_action % 2) == 0;
                    const int magnitude = source >= 16
                        ? std::abs(static_cast<int>(axes[(source - 16) / 2])) : 32767;
                    mapped_joy_axes[axis] += negative ? -magnitude : magnitude;
                    break;
                }
                case kActionJoyButton1: case kActionJoyButton2: case kActionJoyButton3:
                case kActionJoyButton4: case kActionJoyButton5: case kActionJoyButton6:
                case kActionJoyButton7: case kActionJoyButton8:
                    ++mapped_joy_buttons[action - kActionJoyButton1];
                    break;
                case kActionJoyHatUp: mapped_joy_hat |= SDL_HAT_UP; break;
                case kActionJoyHatDown: mapped_joy_hat |= SDL_HAT_DOWN; break;
                case kActionJoyHatLeft: mapped_joy_hat |= SDL_HAT_LEFT; break;
                case kActionJoyHatRight: mapped_joy_hat |= SDL_HAT_RIGHT; break;
                default: break;
                }
            }
        }
    }
    SDL_WineboxJoystickSetEnabled(has_joystick_bindings ? SDL_TRUE : SDL_FALSE);
    for (unsigned axis = 0; axis < mapped_joy_axes.size(); ++axis) {
        const int value = std::clamp(mapped_joy_axes[axis], -32767, 32767);
        SDL_WineboxJoystickSetAxis(static_cast<int>(axis), static_cast<Sint16>(value));
    }
    for (unsigned button = 0; button < mapped_joy_buttons.size(); ++button)
        SDL_WineboxJoystickSetButton(static_cast<int>(button), mapped_joy_buttons[button] ? 1 : 0);
    SDL_WineboxJoystickSetHat(mapped_joy_hat);
    mapped_source_down_ = next_mapped_sources;
    for (std::size_t key = 0; key < mapped_key_counts.size(); ++key)
        mapped_key_down_[key] = mapped_key_counts[key] != 0;
    for (std::size_t button = 0; button < mapped_mouse_counts.size(); ++button)
        mapped_mouse_down_[button] = mapped_mouse_counts[button] != 0;
    sync_all_key_states();

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
        int analog_x = 0, analog_y = 0, dpad_x = 0, dpad_y = 0;
        const bool mouse_mode = controller_mode_ == ControllerMode::MouseLeftStick ||
                                controller_mode_ == ControllerMode::MouseRightStick;
        if (mouse_mode) {
            const unsigned stick = controller_mode_ == ControllerMode::MouseRightStick
                ? RETRO_DEVICE_INDEX_ANALOG_RIGHT : RETRO_DEVICE_INDEX_ANALOG_LEFT;
            analog_x = state_(0, RETRO_DEVICE_ANALOG, stick, RETRO_DEVICE_ID_ANALOG_X);
            analog_y = state_(0, RETRO_DEVICE_ANALOG, stick, RETRO_DEVICE_ID_ANALOG_Y);
            dpad_x = static_cast<int>(button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_RIGHT)) -
                     static_cast<int>(button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_LEFT));
            dpad_y = static_cast<int>(button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_DOWN)) -
                     static_cast<int>(button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_UP));
        }
        const int speed = mapped_speed_up ? 3 : mapped_speed_down ? 1 : 2;
        cursor_x_ += mouse_dx + (std::abs(analog_x) > 4096 ? analog_x / 4096 : 0) + (dpad_x * 8) + mapped_cursor_dx * speed;
        cursor_y_ += mouse_dy + (std::abs(analog_y) > 4096 ? analog_y / 4096 : 0) + (dpad_y * 8) + mapped_cursor_dy * speed;
        cursor_x_ = std::clamp(cursor_x_, 0, static_cast<int>(width_ - 1));
        cursor_y_ = std::clamp(cursor_y_, 0, static_cast<int>(height_ - 1));
    }
    if (overlay_mode_ == OverlayMode::Closed)
        push_mouse_motion(cursor_x_, cursor_y_, width_, height_);

    const bool overlay_closed = overlay_mode_ == OverlayMode::Closed;
    const bool mouse_mode = controller_mode_ == ControllerMode::MouseLeftStick ||
                            controller_mode_ == ControllerMode::MouseRightStick;
    const bool left = overlay_closed && (mapped_mouse_down_[0] || pointer_pressed ||
                      button(state_, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_LEFT) ||
                      (mouse_mode && button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_A)));
    const bool right = overlay_closed && (mapped_mouse_down_[1] ||
                       button(state_, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_RIGHT) ||
                       (mouse_mode && button(state_, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_ID_JOYPAD_B)));
    const bool middle = overlay_closed && (mapped_mouse_down_[2] ||
                        button(state_, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_MIDDLE));
    if (left != left_down_) { push_mouse_button(left, SDL_BUTTON_LEFT, cursor_x_, cursor_y_); left_down_ = left; }
    if (right != right_down_) { push_mouse_button(right, SDL_BUTTON_RIGHT, cursor_x_, cursor_y_); right_down_ = right; }
    if (middle != middle_down_) { push_mouse_button(middle, SDL_BUTTON_MIDDLE, cursor_x_, cursor_y_); middle_down_ = middle; }

    if (overlay_closed && (mapped_wheel_up || button(state_, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_WHEELUP))) {
        SDL_Event event{};
        event.type = SDL_MOUSEWHEEL;
        event.wheel.y = 1;
        push_host_input_event(event);
    }
    if (overlay_closed && (mapped_wheel_down || button(state_, RETRO_DEVICE_MOUSE, RETRO_DEVICE_ID_MOUSE_WHEELDOWN))) {
        SDL_Event event{};
        event.type = SDL_MOUSEWHEEL;
        event.wheel.y = -1;
        push_host_input_event(event);
    }
}

} // namespace winebox
