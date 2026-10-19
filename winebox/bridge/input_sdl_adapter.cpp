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

SDL_Scancode scancode_for_retro(unsigned key) {
    if (key >= RETROK_a && key <= RETROK_z)
        return static_cast<SDL_Scancode>(SDL_SCANCODE_A + key - RETROK_a);
    if (key >= RETROK_1 && key <= RETROK_9)
        return static_cast<SDL_Scancode>(SDL_SCANCODE_1 + key - RETROK_1);
    switch (key) {
    case RETROK_0: return SDL_SCANCODE_0;
    case RETROK_SPACE: return SDL_SCANCODE_SPACE;
    case RETROK_MINUS: return SDL_SCANCODE_MINUS;
    case RETROK_EQUALS: return SDL_SCANCODE_EQUALS;
    case RETROK_LEFTBRACKET: return SDL_SCANCODE_LEFTBRACKET;
    case RETROK_RIGHTBRACKET: return SDL_SCANCODE_RIGHTBRACKET;
    case RETROK_BACKSLASH: return SDL_SCANCODE_BACKSLASH;
    case RETROK_SEMICOLON: return SDL_SCANCODE_SEMICOLON;
    case RETROK_QUOTE: return SDL_SCANCODE_APOSTROPHE;
    case RETROK_BACKQUOTE: return SDL_SCANCODE_GRAVE;
    case RETROK_COMMA: return SDL_SCANCODE_COMMA;
    case RETROK_PERIOD: return SDL_SCANCODE_PERIOD;
    case RETROK_SLASH: return SDL_SCANCODE_SLASH;
    case RETROK_BACKSPACE: return SDL_SCANCODE_BACKSPACE;
    case RETROK_TAB: return SDL_SCANCODE_TAB;
    case RETROK_RETURN: return SDL_SCANCODE_RETURN;
    case RETROK_ESCAPE: return SDL_SCANCODE_ESCAPE;
    case RETROK_DELETE: return SDL_SCANCODE_DELETE;
    case RETROK_PAUSE: return SDL_SCANCODE_PAUSE;
    case RETROK_UP: return SDL_SCANCODE_UP;
    case RETROK_DOWN: return SDL_SCANCODE_DOWN;
    case RETROK_LEFT: return SDL_SCANCODE_LEFT;
    case RETROK_RIGHT: return SDL_SCANCODE_RIGHT;
    case RETROK_INSERT: return SDL_SCANCODE_INSERT;
    case RETROK_HOME: return SDL_SCANCODE_HOME;
    case RETROK_END: return SDL_SCANCODE_END;
    case RETROK_PAGEUP: return SDL_SCANCODE_PAGEUP;
    case RETROK_PAGEDOWN: return SDL_SCANCODE_PAGEDOWN;
    case RETROK_F1: return SDL_SCANCODE_F1;
    case RETROK_F2: return SDL_SCANCODE_F2;
    case RETROK_F3: return SDL_SCANCODE_F3;
    case RETROK_F4: return SDL_SCANCODE_F4;
    case RETROK_F5: return SDL_SCANCODE_F5;
    case RETROK_F6: return SDL_SCANCODE_F6;
    case RETROK_F7: return SDL_SCANCODE_F7;
    case RETROK_F8: return SDL_SCANCODE_F8;
    case RETROK_F9: return SDL_SCANCODE_F9;
    case RETROK_F10: return SDL_SCANCODE_F10;
    case RETROK_F11: return SDL_SCANCODE_F11;
    case RETROK_F12: return SDL_SCANCODE_F12;
    case RETROK_F13: return SDL_SCANCODE_F13;
    case RETROK_F14: return SDL_SCANCODE_F14;
    case RETROK_F15: return SDL_SCANCODE_F15;
    case RETROK_CAPSLOCK: return SDL_SCANCODE_CAPSLOCK;
    case RETROK_CLEAR: return SDL_SCANCODE_CLEAR;
    case RETROK_HELP: return SDL_SCANCODE_HELP;
    case RETROK_PRINT: return SDL_SCANCODE_PRINTSCREEN;
    case RETROK_MENU: return SDL_SCANCODE_MENU;
    case RETROK_MODE: return SDL_SCANCODE_MODE;
    case RETROK_NUMLOCK: return SDL_SCANCODE_NUMLOCKCLEAR;
    case RETROK_SCROLLOCK: return SDL_SCANCODE_SCROLLLOCK;
    case RETROK_LSHIFT: return SDL_SCANCODE_LSHIFT;
    case RETROK_RSHIFT: return SDL_SCANCODE_RSHIFT;
    case RETROK_LCTRL: return SDL_SCANCODE_LCTRL;
    case RETROK_RCTRL: return SDL_SCANCODE_RCTRL;
    case RETROK_LALT: return SDL_SCANCODE_LALT;
    case RETROK_RALT: return SDL_SCANCODE_RALT;
    case RETROK_LMETA: case RETROK_LSUPER: return SDL_SCANCODE_LGUI;
    case RETROK_RMETA: case RETROK_RSUPER: return SDL_SCANCODE_RGUI;
    case RETROK_KP0: return SDL_SCANCODE_KP_0;
    case RETROK_KP1: return SDL_SCANCODE_KP_1;
    case RETROK_KP2: return SDL_SCANCODE_KP_2;
    case RETROK_KP3: return SDL_SCANCODE_KP_3;
    case RETROK_KP4: return SDL_SCANCODE_KP_4;
    case RETROK_KP5: return SDL_SCANCODE_KP_5;
    case RETROK_KP6: return SDL_SCANCODE_KP_6;
    case RETROK_KP7: return SDL_SCANCODE_KP_7;
    case RETROK_KP8: return SDL_SCANCODE_KP_8;
    case RETROK_KP9: return SDL_SCANCODE_KP_9;
    case RETROK_KP_PERIOD: return SDL_SCANCODE_KP_PERIOD;
    case RETROK_KP_DIVIDE: return SDL_SCANCODE_KP_DIVIDE;
    case RETROK_KP_MULTIPLY: return SDL_SCANCODE_KP_MULTIPLY;
    case RETROK_KP_MINUS: return SDL_SCANCODE_KP_MINUS;
    case RETROK_KP_PLUS: return SDL_SCANCODE_KP_PLUS;
    case RETROK_KP_ENTER: return SDL_SCANCODE_KP_ENTER;
    case RETROK_KP_EQUALS: return SDL_SCANCODE_KP_EQUALS;
    default: return SDL_SCANCODE_UNKNOWN;
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
    event.key.keysym.scancode = scancode_for_retro(key);
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

constexpr unsigned kOskMapperButton = kActionJoyHatRight + 1;
struct OskCell { const char* label; unsigned key; unsigned units; };
using OskRow = std::vector<OskCell>;

const std::vector<OskRow>& osk_layout() {
    static const std::vector<OskRow> rows = {
        {{"ESC", RETROK_ESCAPE, 2}, {"F1", RETROK_F1, 1}, {"F2", RETROK_F2, 1},
         {"F3", RETROK_F3, 1}, {"F4", RETROK_F4, 1}, {"F5", RETROK_F5, 1},
         {"F6", RETROK_F6, 1}, {"F7", RETROK_F7, 1}, {"F8", RETROK_F8, 1},
         {"F9", RETROK_F9, 1}, {"F10", RETROK_F10, 1}, {"F11", RETROK_F11, 1},
         {"F12", RETROK_F12, 1}, {"PAD MAPPER", kOskMapperButton, 3}},
        {{"1", RETROK_1, 1}, {"2", RETROK_2, 1}, {"3", RETROK_3, 1}, {"4", RETROK_4, 1},
         {"5", RETROK_5, 1}, {"6", RETROK_6, 1}, {"7", RETROK_7, 1}, {"8", RETROK_8, 1},
         {"9", RETROK_9, 1}, {"0", RETROK_0, 1}, {"-", RETROK_MINUS, 1}, {"=", RETROK_EQUALS, 1},
         {"BACKSPACE", RETROK_BACKSPACE, 2}},
        {{"TAB", RETROK_TAB, 2}, {"Q", RETROK_q, 1}, {"W", RETROK_w, 1}, {"E", RETROK_e, 1},
         {"R", RETROK_r, 1}, {"T", RETROK_t, 1}, {"Y", RETROK_y, 1}, {"U", RETROK_u, 1},
         {"I", RETROK_i, 1}, {"O", RETROK_o, 1}, {"P", RETROK_p, 1}, {"[", RETROK_LEFTBRACKET, 1},
         {"]", RETROK_RIGHTBRACKET, 1}, {"\\", RETROK_BACKSLASH, 1}},
        {{"CAPS", RETROK_CAPSLOCK, 2}, {"A", RETROK_a, 1}, {"S", RETROK_s, 1}, {"D", RETROK_d, 1},
         {"F", RETROK_f, 1}, {"G", RETROK_g, 1}, {"H", RETROK_h, 1}, {"J", RETROK_j, 1},
         {"K", RETROK_k, 1}, {"L", RETROK_l, 1}, {";", RETROK_SEMICOLON, 1}, {"'", RETROK_QUOTE, 1},
         {"ENTER", RETROK_RETURN, 2}},
        {{"SHIFT", RETROK_LSHIFT, 2}, {"Z", RETROK_z, 1}, {"X", RETROK_x, 1}, {"C", RETROK_c, 1},
         {"V", RETROK_v, 1}, {"B", RETROK_b, 1}, {"N", RETROK_n, 1}, {"M", RETROK_m, 1},
         {",", RETROK_COMMA, 1}, {".", RETROK_PERIOD, 1}, {"/", RETROK_SLASH, 1}, {"SHIFT", RETROK_RSHIFT, 2}},
        {{"CTRL", RETROK_LCTRL, 2}, {"ALT", RETROK_LALT, 2}, {"SPACE", RETROK_SPACE, 5},
         {"ALT", RETROK_RALT, 2}, {"CTRL", RETROK_RCTRL, 2}, {"LEFT", RETROK_LEFT, 1},
         {"DOWN", RETROK_DOWN, 1}, {"UP", RETROK_UP, 1}, {"RIGHT", RETROK_RIGHT, 1}},
        {{"INS", RETROK_INSERT, 1}, {"HOME", RETROK_HOME, 1}, {"PGUP", RETROK_PAGEUP, 1},
         {"DEL", RETROK_DELETE, 1}, {"END", RETROK_END, 1}, {"PGDN", RETROK_PAGEDOWN, 1},
         {"PRINT", RETROK_PRINT, 1}, {"PAUSE", RETROK_PAUSE, 1}, {"NUM", RETROK_NUMLOCK, 1},
         {"KP /", RETROK_KP_DIVIDE, 1}, {"KP *", RETROK_KP_MULTIPLY, 1}, {"KP -", RETROK_KP_MINUS, 1}},
        {{"KP 7", RETROK_KP7, 1}, {"KP 8", RETROK_KP8, 1}, {"KP 9", RETROK_KP9, 1},
         {"KP +", RETROK_KP_PLUS, 1}, {"KP 4", RETROK_KP4, 1}, {"KP 5", RETROK_KP5, 1},
         {"KP 6", RETROK_KP6, 1}, {"KP 1", RETROK_KP1, 1}, {"KP 2", RETROK_KP2, 1},
         {"KP 3", RETROK_KP3, 1}, {"KP 0", RETROK_KP0, 2}, {"KP .", RETROK_KP_PERIOD, 1},
         {"KP ENTER", RETROK_KP_ENTER, 2}}
    };
    return rows;
}

std::vector<unsigned> assign_options(unsigned category) {
    std::vector<unsigned> result{0};
    for (const auto& entry : osk_keys(true)) {
        const unsigned value = entry.second;
        if (!value)
            continue;
        const bool keyboard = value < RETROK_LAST;
        const bool mouse = value >= kActionMouseLeft && value <= kActionMouseSpeedDown;
        const bool joystick = value >= kActionJoy1AxisLeft && value <= kActionJoyHatRight;
        if ((category == 0 && keyboard) || (category == 1 && mouse) || (category == 2 && joystick))
            result.push_back(value);
    }
    return result;
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

struct UiRect { int x, y, w, h; };
void draw_text(std::uint32_t* pixels, unsigned width, unsigned height, int x, int y,
               const std::string& text, std::uint32_t color, int scale);

UiRect ui_panel(unsigned width, unsigned height) {
    const int margin = std::max(8, static_cast<int>(std::min(width, height)) / 40);
    return {margin, margin, std::max(1, static_cast<int>(width) - margin * 2),
            std::max(1, static_cast<int>(height) - margin * 2)};
}

UiRect ui_tabs(const UiRect& panel) {
    return {panel.x + 12, panel.y + 10, panel.w - 24, std::max(28, panel.h / 12)};
}

UiRect ui_content(const UiRect& panel) {
    const UiRect tabs = ui_tabs(panel);
    return {panel.x + 14, tabs.y + tabs.h + 10, panel.w - 28,
            std::max(1, panel.h - (tabs.y - panel.y) - tabs.h - 34)};
}

UiRect ui_touch_trigger(unsigned width, unsigned height) {
    constexpr int trigger_w = 88, trigger_h = 34, margin = 8;
    return {std::max(0, static_cast<int>(width) - trigger_w - margin),
            std::max(0, static_cast<int>(height) - trigger_h - margin), trigger_w, trigger_h};
}

void draw_ui_button(std::uint32_t* pixels, unsigned width, unsigned height,
                    const UiRect& r, const std::string& label, bool selected,
                    int scale = 2) {
    const std::uint32_t fill = selected ? 0x003C2A15u : 0x001A222Bu;
    const std::uint32_t edge = selected ? 0x00FFB23Fu : 0x00485A6Au;
    draw_rect(pixels, width, height, r.x, r.y, r.w, r.h, fill);
    draw_rect(pixels, width, height, r.x, r.y, r.w, 2, edge);
    draw_rect(pixels, width, height, r.x, r.y, 2, r.h, edge);
    draw_rect(pixels, width, height, r.x, r.y + r.h - 2, r.w, 2, 0x00080C10u);
    draw_rect(pixels, width, height, r.x + r.w - 2, r.y, 2, r.h, 0x00080C10u);
    const int max_chars = std::max(1, (r.w - 12) / (4 * scale));
    std::string text = label;
    if (static_cast<int>(text.size()) > max_chars)
        text.resize(static_cast<std::size_t>(max_chars));
    const int tw = static_cast<int>(text.size()) * 4 * scale;
    draw_text(pixels, width, height, r.x + std::max(5, (r.w - tw) / 2),
              r.y + std::max(3, (r.h - 5 * scale) / 2), text,
              selected ? 0x00FFF0D3u : 0x00D6DFE7u, scale);
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
                                           const std::array<std::int16_t, 4>& axes,
                                           bool touch_pressed, int touch_x, int touch_y) {
    const bool was_closed = overlay_mode_ == OverlayMode::Closed;
    const bool was_open = !was_closed;
    std::array<bool, 16> pressed{};
    for (std::size_t i = 0; i < buttons.size(); ++i)
        pressed[i] = buttons[i] && !ui_button_previous_[i];
    const std::array<bool, 4> axis_active = {axes[0] < -12000, axes[0] > 12000,
                                             axes[1] < -12000, axes[1] > 12000};
    bool nav_left = pressed[RETRO_DEVICE_ID_JOYPAD_LEFT] || (axis_active[0] && !ui_axis_previous_[0]);
    bool nav_right = pressed[RETRO_DEVICE_ID_JOYPAD_RIGHT] || (axis_active[1] && !ui_axis_previous_[1]);
    bool nav_up = pressed[RETRO_DEVICE_ID_JOYPAD_UP] || (axis_active[2] && !ui_axis_previous_[2]);
    bool nav_down = pressed[RETRO_DEVICE_ID_JOYPAD_DOWN] || (axis_active[3] && !ui_axis_previous_[3]);
    bool accept = pressed[RETRO_DEVICE_ID_JOYPAD_A] || pressed[RETRO_DEVICE_ID_JOYPAD_START];
    bool cancel = pressed[RETRO_DEVICE_ID_JOYPAD_B] || pressed[RETRO_DEVICE_ID_JOYPAD_SELECT];
    const bool touch_click = touch_pressed && !ui_pointer_previous_;
    bool touch_accept = false;

    if (overlay_mode_ == OverlayMode::Closed) {
        if (pressed[RETRO_DEVICE_ID_JOYPAD_L3]) {
            overlay_mode_ = OverlayMode::Menu;
            menu_selection_ = 0;
        } else if (touch_click) {
            const UiRect trigger = ui_touch_trigger(width_, height_);
            if (touch_x >= trigger.x && touch_x < trigger.x + trigger.w &&
                touch_y >= trigger.y && touch_y < trigger.y + trigger.h) {
                overlay_mode_ = OverlayMode::Menu;
                menu_selection_ = 0;
            }
        }
    } else if (pressed[RETRO_DEVICE_ID_JOYPAD_L3]) {
        overlay_mode_ = OverlayMode::Closed;
    }

    if (touch_click && overlay_mode_ != OverlayMode::Closed) {
        const UiRect panel = ui_panel(width_, height_);
        const UiRect tabs = ui_tabs(panel);
        const UiRect content = ui_content(panel);
        if (touch_y >= tabs.y && touch_y < tabs.y + tabs.h &&
            touch_x >= tabs.x && touch_x < tabs.x + tabs.w) {
            const unsigned tab = std::min(2u, static_cast<unsigned>(touch_x - tabs.x) * 3u /
                                               static_cast<unsigned>(std::max(1, tabs.w)));
            overlay_mode_ = tab == 0 ? OverlayMode::Menu : tab == 1 ? OverlayMode::Keyboard : OverlayMode::Mapper;
            if (overlay_mode_ == OverlayMode::Menu) menu_selection_ = 0;
            if (overlay_mode_ == OverlayMode::Keyboard) keyboard_selection_ = 0;
        } else if (overlay_mode_ == OverlayMode::Menu) {
            const int row_h = std::max(1, content.h / 3);
            menu_selection_ = std::min(2u, static_cast<unsigned>(std::max(0, touch_y - content.y) / row_h));
            touch_accept = true;
        } else if (overlay_mode_ == OverlayMode::Mapper) {
            const unsigned columns = height_ < 400 ? 4u : 3u;
            const unsigned rows = 24u / columns;
            const int top = content.y + 38;
            const int row_h = std::max(1, (content.h - 44 - static_cast<int>(rows - 1) * 6) / static_cast<int>(rows));
            const int col_w = std::max(1, content.w / static_cast<int>(columns));
            const unsigned row = std::min(rows - 1, static_cast<unsigned>(std::max(0, touch_y - top) / row_h));
            const unsigned col = std::min(columns - 1, static_cast<unsigned>(std::max(0, touch_x - content.x) / col_w));
            selected_control_ = std::min(23u, row * columns + col);
            touch_accept = true;
        } else if (overlay_mode_ == OverlayMode::MapperSlots) {
            const int top = content.y + 58;
            const int row_h = std::max(1, (content.h - 68 - 3 * 8) / 4);
            selected_action_slot_ = std::min(3u, static_cast<unsigned>(std::max(0, touch_y - top) / row_h));
            touch_accept = true;
        } else if (overlay_mode_ == OverlayMode::Keyboard) {
            const auto& rows = osk_layout();
            const int top = content.y + 6;
            const int row_h = std::max(1, (content.h - 12) / static_cast<int>(rows.size()));
            if (touch_y >= top && touch_y < top + row_h * static_cast<int>(rows.size())) {
                const unsigned row = std::min(static_cast<unsigned>(rows.size() - 1),
                    static_cast<unsigned>((touch_y - top) / row_h));
                unsigned units = 0;
                for (const OskCell& cell : rows[row]) units += cell.units;
                const int unit_w = std::max(1, content.w / static_cast<int>(units));
                int x = content.x;
                unsigned flat = 0;
                for (unsigned r = 0; r < row; ++r) flat += static_cast<unsigned>(rows[r].size());
                for (const OskCell& cell : rows[row]) {
                    const int w = unit_w * static_cast<int>(cell.units);
                    if (touch_x >= x && touch_x < x + w) {
                        keyboard_selection_ = flat;
                        touch_accept = true;
                        break;
                    }
                    x += w;
                    ++flat;
                }
            }
        } else if (overlay_mode_ == OverlayMode::AssignKeyboard) {
            const int tab_h = 42;
            if (touch_y >= content.y && touch_y < content.y + tab_h) {
                assign_category_ = std::min(2u, static_cast<unsigned>(std::max(0, touch_x - content.x)) * 3u /
                                                   static_cast<unsigned>(std::max(1, content.w)));
                keyboard_selection_ = 0;
            } else {
                const auto options = assign_options(assign_category_);
                constexpr unsigned columns = 5;
                const int grid_y = content.y + tab_h + 6;
                const int cell_h = 42;
                const unsigned visible_rows = std::max(1, (content.h - tab_h - 12) / cell_h);
                const unsigned total_rows = static_cast<unsigned>((options.size() + columns - 1) / columns);
                const unsigned selected_row = keyboard_selection_ / columns;
                const unsigned first_row = std::min(total_rows > visible_rows ? total_rows - visible_rows : 0,
                    selected_row > visible_rows / 2 ? selected_row - visible_rows / 2 : 0);
                const unsigned row = first_row + std::min(visible_rows - 1,
                    static_cast<unsigned>(std::max(0, touch_y - grid_y) / cell_h));
                const unsigned col = std::min(columns - 1,
                    static_cast<unsigned>(std::max(0, touch_x - content.x)) * columns /
                    static_cast<unsigned>(std::max(1, content.w)));
                const unsigned index = row * columns + col;
                if (index < options.size()) {
                    keyboard_selection_ = index;
                    touch_accept = true;
                }
            }
        }
    }

    if (overlay_mode_ == OverlayMode::Menu) {
        if (nav_up) menu_selection_ = (menu_selection_ + 2) % 3;
        if (nav_down) menu_selection_ = (menu_selection_ + 1) % 3;
        if (cancel) overlay_mode_ = OverlayMode::Closed;
        if (accept || touch_accept) {
            if (menu_selection_ == 0) overlay_mode_ = OverlayMode::Closed;
            else if (menu_selection_ == 1) overlay_mode_ = OverlayMode::Mapper;
            else overlay_mode_ = OverlayMode::Keyboard;
        }
    } else if (overlay_mode_ == OverlayMode::Mapper) {
        if (pressed[RETRO_DEVICE_ID_JOYPAD_L]) overlay_mode_ = OverlayMode::Keyboard;
        if (pressed[RETRO_DEVICE_ID_JOYPAD_R]) overlay_mode_ = OverlayMode::Keyboard;
        const unsigned columns = height_ < 400 ? 4u : 3u;
        const unsigned column = selected_control_ % columns;
        const unsigned row_start = (selected_control_ / columns) * columns;
        if (nav_left) selected_control_ = row_start + (column + columns - 1) % columns;
        if (nav_right) selected_control_ = row_start + (column + 1) % columns;
        if (nav_up) selected_control_ = (selected_control_ + 24 - columns) % 24;
        if (nav_down) selected_control_ = (selected_control_ + columns) % 24;
        if (cancel) overlay_mode_ = OverlayMode::Menu;
        if ((accept || touch_accept) && selected_control_ != RETRO_DEVICE_ID_JOYPAD_L3) {
            overlay_mode_ = OverlayMode::MapperSlots;
            selected_action_slot_ = 0;
        }
    } else if (overlay_mode_ == OverlayMode::MapperSlots) {
        if (nav_up) selected_action_slot_ = (selected_action_slot_ + 3) % 4;
        if (nav_down) selected_action_slot_ = (selected_action_slot_ + 1) % 4;
        if (cancel) overlay_mode_ = OverlayMode::Mapper;
        if (pressed[RETRO_DEVICE_ID_JOYPAD_X]) {
            mapping_[selected_control_][selected_action_slot_] = 0;
            save_mapping();
            sync_virtual_joystick();
        }
        if (accept || touch_accept) {
            overlay_mode_ = OverlayMode::AssignKeyboard;
            keyboard_selection_ = 0;
            assign_category_ = 0;
        }
    } else if (overlay_mode_ == OverlayMode::Keyboard) {
        if (pressed[RETRO_DEVICE_ID_JOYPAD_L]) overlay_mode_ = OverlayMode::Menu;
        if (pressed[RETRO_DEVICE_ID_JOYPAD_R]) overlay_mode_ = OverlayMode::Mapper;
        const auto& rows = osk_layout();
        unsigned row = 0, col = keyboard_selection_;
        while (row + 1 < rows.size() && col >= rows[row].size()) {
            col -= static_cast<unsigned>(rows[row].size());
            ++row;
        }
        if (nav_left) col = (col + static_cast<unsigned>(rows[row].size()) - 1) % static_cast<unsigned>(rows[row].size());
        if (nav_right) col = (col + 1) % static_cast<unsigned>(rows[row].size());
        if (nav_up || nav_down) {
            const unsigned next_row = nav_up ? (row + static_cast<unsigned>(rows.size()) - 1) % static_cast<unsigned>(rows.size())
                                             : (row + 1) % static_cast<unsigned>(rows.size());
            col = std::min(col, static_cast<unsigned>(rows[next_row].size() - 1));
            row = next_row;
        }
        keyboard_selection_ = col;
        for (unsigned r = 0; r < row; ++r) keyboard_selection_ += static_cast<unsigned>(rows[r].size());
        if (cancel) overlay_mode_ = OverlayMode::Menu;
        if (accept || touch_accept) {
            unsigned flat = 0;
            const OskCell* selected = nullptr;
            for (const OskRow& line : rows) {
                if (keyboard_selection_ < flat + line.size()) {
                    selected = &line[keyboard_selection_ - flat];
                    break;
                }
                flat += static_cast<unsigned>(line.size());
            }
            if (selected && selected->key == kOskMapperButton) {
                overlay_mode_ = OverlayMode::Mapper;
            } else if (selected && selected->key) {
                if (selected->key == RETROK_LSHIFT || selected->key == RETROK_RSHIFT ||
                    selected->key == RETROK_LCTRL || selected->key == RETROK_RCTRL ||
                    selected->key == RETROK_LALT || selected->key == RETROK_RALT) {
                    const unsigned key = selected->key;
                    if (osk_latched_modifier_ == key) {
                        osk_latched_modifier_ = 0;
                        update_sdl_modifiers();
                        push_key_event(false, key);
                    } else {
                        if (osk_latched_modifier_) push_key_event(false, osk_latched_modifier_);
                        osk_latched_modifier_ = key;
                        update_sdl_modifiers();
                        push_key_event(true, key);
                    }
                } else {
                    push_key_event(true, selected->key);
                    push_key_event(false, selected->key);
                }
            }
        }
    } else if (overlay_mode_ == OverlayMode::AssignKeyboard) {
        if (pressed[RETRO_DEVICE_ID_JOYPAD_L]) assign_category_ = (assign_category_ + 2) % 3;
        if (pressed[RETRO_DEVICE_ID_JOYPAD_R]) assign_category_ = (assign_category_ + 1) % 3;
        const auto options = assign_options(assign_category_);
        const unsigned count = static_cast<unsigned>(options.size());
        constexpr unsigned columns = 5;
        if (nav_left) keyboard_selection_ = (keyboard_selection_ + count - 1) % count;
        if (nav_right) keyboard_selection_ = (keyboard_selection_ + 1) % count;
        if (nav_up) keyboard_selection_ = (keyboard_selection_ + count - std::min(columns, count)) % count;
        if (nav_down) keyboard_selection_ = (keyboard_selection_ + std::min(columns, count)) % count;
        if (cancel) overlay_mode_ = OverlayMode::MapperSlots;
        if (accept || touch_accept) {
            mapping_[selected_control_][selected_action_slot_] = options[keyboard_selection_];
            save_mapping();
            sync_virtual_joystick();
            overlay_mode_ = OverlayMode::MapperSlots;
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
        key_down_.fill(false);
        callback_key_down_.fill(false);
    }
    ui_button_previous_ = buttons;
    ui_axis_previous_ = axis_active;
    ui_pointer_previous_ = touch_pressed;
}

bool SdlInputAdapter::overlay_active() const noexcept {
    return overlay_mode_ != OverlayMode::Closed;
}

void SdlInputAdapter::render_overlay(std::uint32_t* pixels, unsigned width, unsigned height) const {
    if (!pixels || !width || !height)
        return;
    if (overlay_mode_ == OverlayMode::Closed) {
        const UiRect trigger = ui_touch_trigger(width, height);
        draw_ui_button(pixels, width, height, trigger, "MENU", false, 1);
        return;
    }

    const UiRect panel = ui_panel(width, height);
    const UiRect tabs = ui_tabs(panel);
    const UiRect content = ui_content(panel);
    draw_rect(pixels, width, height, panel.x, panel.y, panel.w, panel.h, 0x00090D12u);
    draw_rect(pixels, width, height, panel.x, panel.y, panel.w, 3, 0x00C77C28u);
    draw_rect(pixels, width, height, panel.x, panel.y, 3, panel.h, 0x00C77C28u);
    draw_rect(pixels, width, height, panel.x, panel.y + panel.h - 3, panel.w, 3, 0x00C77C28u);
    draw_rect(pixels, width, height, panel.x + panel.w - 3, panel.y, 3, panel.h, 0x00C77C28u);

    const std::array<const char*, 3> tab_labels = {"START MENU", "ON-SCREEN KEYBOARD", "CONTROLLER MAPPER"};
    const unsigned active_tab = (overlay_mode_ == OverlayMode::Keyboard) ? 1
        : (overlay_mode_ == OverlayMode::Mapper || overlay_mode_ == OverlayMode::MapperSlots ||
           overlay_mode_ == OverlayMode::AssignKeyboard) ? 2 : 0;
    const int tab_w = tabs.w / 3;
    for (unsigned i = 0; i < tab_labels.size(); ++i) {
        const UiRect r{tabs.x + static_cast<int>(i) * tab_w, tabs.y,
                       i == 2 ? tabs.w - 2 * tab_w : tab_w - 4, tabs.h};
        draw_ui_button(pixels, width, height, r, tab_labels[i], i == active_tab, 1);
    }

    const int footer_y = panel.y + panel.h - 27;
    draw_rect(pixels, width, height, panel.x + 8, footer_y - 4, panel.w - 16, 1, 0x00475A6A);
    std::string footer = "D-PAD / STICK NAVIGATE     A SELECT     B BACK     TAP TO SELECT";
    if (overlay_mode_ == OverlayMode::Keyboard)
        footer = "A TYPE     B BACK     L/R CHANGE TAB     TOUCH ANY KEY";
    else if (overlay_mode_ == OverlayMode::MapperSlots)
        footer = "A REBIND     X CLEAR     B BACK     TAP A BINDING";
    else if (overlay_mode_ == OverlayMode::AssignKeyboard)
        footer = "A ASSIGN     L/R CATEGORY     B CANCEL     TAP A TARGET";
    draw_text(pixels, width, height, panel.x + 14, footer_y + 7, footer, 0x00AFC0CCu, 1);

    if (overlay_mode_ == OverlayMode::Menu) {
        const bool compact = height < 400;
        draw_text(pixels, width, height, content.x + 8, content.y + 5,
                  "WINEBOX CONTROL CENTER", 0x00F4A94Au, compact ? 1 : 2);
        const std::array<const char*, 3> labels = {"RESUME GAME", "CONTROLLER MAPPER", "ON-SCREEN KEYBOARD"};
        const int gap = std::max(4, content.h / 36);
        const int top = content.y + (compact ? 24 : 52);
        const int row_h = std::max(compact ? 25 : 38, (content.h - (compact ? 30 : 62) - 2 * gap) / 3);
        for (unsigned i = 0; i < labels.size(); ++i) {
            const UiRect r{content.x + 8, top + static_cast<int>(i) * (row_h + gap), content.w - 16, row_h};
            draw_ui_button(pixels, width, height, r, labels[i], i == menu_selection_, compact ? 1 : 2);
        }
        if (!compact)
            draw_text(pixels, width, height, content.x + 10, content.y + content.h - 24,
                      "L3 CLOSES MENU. TOUCH OR CONTROLLER WORKS WITHOUT A MOUSE.", 0x008B9CA9u, 1);
        return;
    }

    if (overlay_mode_ == OverlayMode::Mapper) {
        static const std::array<const char*, 24> sources = {
            "B", "Y", "SELECT", "START", "DPAD UP", "DPAD DOWN", "DPAD LEFT", "DPAD RIGHT",
            "A", "X", "L", "R", "L2", "R2", "L3 MENU", "R3",
            "LEFT X -", "LEFT X +", "LEFT Y -", "LEFT Y +", "RIGHT X -", "RIGHT X +", "RIGHT Y -", "RIGHT Y +"};
        draw_text(pixels, width, height, content.x + 8, content.y + 7,
                  "PORT 1     SELECT A CONTROL TO EDIT FOUR BINDINGS", 0x00F4A94Au, 1);
        const int top = content.y + 38;
        const int gap = 6;
        const unsigned columns = height < 400 ? 4u : 3u;
        const unsigned rows = 24u / columns;
        const int col_w = (content.w - static_cast<int>(columns - 1) * gap) / static_cast<int>(columns);
        const int row_h = (content.h - 44 - static_cast<int>(rows - 1) * gap) / static_cast<int>(rows);
        for (unsigned i = 0; i < 24; ++i) {
            const unsigned row = i / columns, col = i % columns;
            const UiRect r{content.x + static_cast<int>(col) * (col_w + gap),
                           top + static_cast<int>(row) * (row_h + gap), col_w, row_h};
            draw_ui_button(pixels, width, height, r, "", i == selected_control_, 1);
            draw_text(pixels, width, height, r.x + 8, r.y + 5, sources[i],
                      i == selected_control_ ? 0x00FFF0D3u : 0x00D1DCE5u, 1);
            std::string binding;
            unsigned shown = 0;
            for (unsigned action : mapping_[i]) {
                if (!action) continue;
                if (shown == 0) binding = action_name(action);
                ++shown;
            }
            if (!shown) binding = "UNBOUND";
            else if (shown > 1) binding += "  +" + std::to_string(shown - 1);
            if (height >= 400)
                draw_text(pixels, width, height, r.x + 8, r.y + row_h - 12, binding,
                          i == selected_control_ ? 0x00F4A94Au : 0x008FA2B1u, 1);
        }
        return;
    }

    if (overlay_mode_ == OverlayMode::MapperSlots) {
        static const std::array<const char*, 24> sources = {
            "B", "Y", "SELECT", "START", "DPAD UP", "DPAD DOWN", "DPAD LEFT", "DPAD RIGHT",
            "A", "X", "L", "R", "L2", "R2", "L3 MENU", "R3",
            "LEFT X -", "LEFT X +", "LEFT Y -", "LEFT Y +", "RIGHT X -", "RIGHT X +", "RIGHT Y -", "RIGHT Y +"};
        draw_text(pixels, width, height, content.x + 8, content.y + 5,
                  "REBIND CONTROL: " + std::string(sources[selected_control_]), 0x00F4A94Au, 2);
        draw_text(pixels, width, height, content.x + 8, content.y + 31,
                  "UP TO FOUR ACTIONS RUN TOGETHER WHEN THIS CONTROL IS HELD", 0x008FA2B1u, 1);
        const int top = content.y + 58;
        const int gap = 8;
        const int row_h = (content.h - 68 - 3 * gap) / 4;
        for (unsigned slot = 0; slot < 4; ++slot) {
            const UiRect r{content.x + 8, top + static_cast<int>(slot) * (row_h + gap), content.w - 16, row_h};
            const std::string label = "ACTION " + std::to_string(slot + 1) + "     " +
                (mapping_[selected_control_][slot] ? action_name(mapping_[selected_control_][slot]) : "EMPTY - TAP TO BIND");
            draw_ui_button(pixels, width, height, r, label, slot == selected_action_slot_, height < 400 ? 1 : 2);
        }
        return;
    }

    if (overlay_mode_ == OverlayMode::Keyboard) {
        const auto& rows = osk_layout();
        unsigned flat = 0;
        unsigned selected_row = 0, selected_col = 0;
        for (unsigned row = 0; row < rows.size(); ++row) {
            if (keyboard_selection_ < flat + rows[row].size()) {
                selected_row = row;
                selected_col = keyboard_selection_ - flat;
                break;
            }
            flat += static_cast<unsigned>(rows[row].size());
        }
        const int top = content.y + 6;
        const int gap = std::max(3, content.h / 100);
        const int row_h = (content.h - 12 - gap * static_cast<int>(rows.size() - 1)) /
                          static_cast<int>(rows.size());
        flat = 0;
        for (unsigned row = 0; row < rows.size(); ++row) {
            unsigned units = 0;
            for (const OskCell& cell : rows[row]) units += cell.units;
            const int unit_w = content.w / static_cast<int>(units);
            int x = content.x;
            for (unsigned col = 0; col < rows[row].size(); ++col) {
                const OskCell& cell = rows[row][col];
                const int cell_w = unit_w * static_cast<int>(cell.units);
                const UiRect r{x + 1, top + static_cast<int>(row) * (row_h + gap), cell_w - 3, row_h};
                draw_ui_button(pixels, width, height, r, cell.label,
                               (row == selected_row && col == selected_col) ||
                                   (osk_latched_modifier_ != 0 && cell.key == osk_latched_modifier_),
                               height < 400 || cell.units > 1 ? 1 : 2);
                x += cell_w;
                ++flat;
            }
        }
        return;
    }

    // The rebinding target picker is grouped by destination so it remains usable
    // on a phone-sized framebuffer and without a pointing device.
    const std::array<const char*, 3> categories = {"KEYBOARD", "MOUSE", "JOYSTICK"};
    const int category_gap = 6;
    const int category_w = (content.w - 2 * category_gap) / 3;
    for (unsigned i = 0; i < categories.size(); ++i) {
        const UiRect r{content.x + static_cast<int>(i) * (category_w + category_gap), content.y,
                       category_w, 40};
        draw_ui_button(pixels, width, height, r, categories[i], i == assign_category_, 1);
    }
    const auto options = assign_options(assign_category_);
    constexpr unsigned columns = 5;
    const int grid_y = content.y + 48;
    const int cell_h = 42;
    const unsigned visible_rows = std::max(1, (content.h - 52) / cell_h);
    const unsigned total_rows = static_cast<unsigned>((options.size() + columns - 1) / columns);
    const unsigned selected_row = keyboard_selection_ / columns;
    const unsigned first_row = std::min(total_rows > visible_rows ? total_rows - visible_rows : 0,
        selected_row > visible_rows / 2 ? selected_row - visible_rows / 2 : 0);
    const unsigned end_row = std::min(total_rows, first_row + visible_rows);
    const int cell_w = content.w / static_cast<int>(columns);
    for (unsigned row = first_row; row < end_row; ++row) {
        for (unsigned col = 0; col < columns; ++col) {
            const unsigned i = row * columns + col;
            if (i >= options.size()) break;
            const UiRect r{content.x + static_cast<int>(col) * cell_w + 2,
                           grid_y + static_cast<int>(row - first_row) * cell_h,
                           cell_w - 4, cell_h - 3};
            draw_ui_button(pixels, width, height, r, action_name(options[i]), i == keyboard_selection_, 1);
        }
    }
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
    ui_pointer_previous_ = false;
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
    const std::int16_t pointer_x = state_(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_X);
    const std::int16_t pointer_y = state_(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_Y);
    const bool pointer_pressed = state_(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_PRESSED) != 0;
    const int touch_x = static_cast<int>((static_cast<std::int32_t>(pointer_x) + 32768) * (width_ - 1) / 65535);
    const int touch_y = static_cast<int>((static_cast<std::int32_t>(pointer_y) + 32768) * (height_ - 1) / 65535);
    handle_overlay_input(current_buttons, axes, pointer_pressed, touch_x, touch_y);

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
