#include "../bridge/audio_adapter.hpp"
#include "../bridge/boxedwine_hooks.hpp"
#include "../bridge/input_sdl_adapter.hpp"
#include "../bridge/retro_host.hpp"

#include <SDL.h>
#include <libretro.h>

#include <atomic>
#include <chrono>
#include <cctype>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <limits>
#include <memory>
#include <string>
#include <thread>
#include <utility>
#include <vector>

// Implemented in the BoxedWine SDL platform patch. It posts SDL_QUIT through
// BoxedWine's own platform entry point so unload can join the guest cleanly.
extern "C" void winebox_boxedwine_request_quit();
extern int boxedmain(int argc, const char** argv);

namespace {

constexpr unsigned kInitialWidth = 800;
constexpr unsigned kInitialHeight = 600;
constexpr unsigned kAudioFramesPerRun = 735; // 44.1 kHz at 60 Hz.
constexpr auto kVideoStatsInterval = std::chrono::seconds(5);
constexpr const char* kSystemSubdir = "winebox";
constexpr const char* kRootArchive = "TinyCore15Wine11.0.zip";
constexpr const char* kProjectorExe = "FlashPlayer.exe";

retro_environment_t g_environment = nullptr;
retro_video_refresh_t g_video = nullptr;
retro_audio_sample_t g_audio_sample = nullptr;
retro_audio_sample_batch_t g_audio_batch = nullptr;
retro_input_poll_t g_input_poll = nullptr;
retro_input_state_t g_input_state = nullptr;
retro_log_printf_t g_log = nullptr;
std::unique_ptr<winebox::RetroHost> g_host;
std::unique_ptr<winebox::SdlInputAdapter> g_input;
std::thread g_guest;
std::atomic<bool> g_guest_finished{true};
bool g_initialized = false;
bool g_pixel_format_supported = false;
bool g_game_loaded = false;
bool g_exit_reported = false;
int g_guest_exit_code = 0;
std::vector<std::string> g_guest_args;
std::string g_save_dir;
std::chrono::steady_clock::time_point g_video_stats_started{};
std::uint64_t g_video_stats_publications = 0;
std::uint64_t g_video_stats_publish_nanoseconds = 0;
std::uint64_t g_video_stats_present_calls = 0;
double g_video_stats_present_milliseconds = 0.0;

void log_message(enum retro_log_level level, const char* message) {
    if (g_log && message)
        g_log(level, "%s\n", message);
}

std::size_t RETRO_CALLCONV single_sample_batch(const std::int16_t* samples, std::size_t frames) {
    if (!g_audio_sample || !samples)
        return 0;
    for (std::size_t frame = 0; frame < frames; ++frame)
        g_audio_sample(samples[frame * 2], samples[frame * 2 + 1]);
    return frames;
}

bool get_variable(const char* key, std::string& value) {
    if (!g_environment)
        return false;
    retro_variable variable{key, nullptr};
    if (!g_environment(RETRO_ENVIRONMENT_GET_VARIABLE, &variable) || !variable.value)
        return false;
    value = variable.value;
    return true;
}

bool regular_file(const std::filesystem::path& path) {
    std::error_code error;
    return std::filesystem::is_regular_file(path, error) && !error;
}

bool safe_leaf_name(const std::string& value) {
    if (value.empty() || value == "." || value == "..")
        return false;
    return std::filesystem::path(value).filename() == std::filesystem::path(value);
}

std::string mapping_file_for_content(const char* path) {
    std::string stem = path ? std::filesystem::u8path(path).stem().string() : std::string{};
    for (char& character : stem) {
        const auto byte = static_cast<unsigned char>(character);
        if (!std::isalnum(byte) && character != '-' && character != '_')
            character = '_';
    }
    if (stem.empty())
        stem = "default";
    return (std::filesystem::u8path(g_save_dir) / ("controller-" + stem + ".ini")).string();
}

bool prepare_launch(const retro_game_info* game) {
    if (!game || !g_environment)
        return false;

    const char* system_directory = nullptr;
    const char* save_directory = nullptr;
    if (!g_environment(RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY, &system_directory) || !system_directory) {
        log_message(RETRO_LOG_ERROR, "Winebox could not get RetroArch's system directory.");
        return false;
    }
    if (!g_environment(RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY, &save_directory) || !save_directory) {
        log_message(RETRO_LOG_ERROR, "Winebox could not get RetroArch's save directory; configure Save Files in Settings > Directory.");
        return false;
    }

    const std::filesystem::path system_root = std::filesystem::u8path(system_directory) / kSystemSubdir;
    const std::filesystem::path save_root = std::filesystem::u8path(save_directory) / "winebox";
    std::error_code error;
    std::filesystem::create_directories(save_root, error);
    if (error) {
        log_message(RETRO_LOG_ERROR, "Winebox could not create its writable guest root directory.");
        return false;
    }

    std::string root_leaf = kRootArchive;
    std::string projector_leaf = kProjectorExe;
    std::string option;
    if (get_variable("winebox_root", option) && safe_leaf_name(option))
        root_leaf = option;
    if (get_variable("winebox_projector", option) && safe_leaf_name(option))
        projector_leaf = option;

    const std::filesystem::path root_archive = system_root / root_leaf;
    const std::filesystem::path projector = system_root / projector_leaf;
    if (!regular_file(root_archive)) {
        log_message(RETRO_LOG_ERROR, ("Winebox root archive is missing or not a regular file: " + root_archive.string()).c_str());
        return false;
    }
    if (!regular_file(projector)) {
        log_message(RETRO_LOG_ERROR, ("Winebox Flash projector is missing or not a regular file: " + projector.string()).c_str());
        return false;
    }
    std::filesystem::path swf;
    if (game->path) {
        const auto requested_path = std::filesystem::u8path(game->path);
        if (regular_file(requested_path))
            swf = requested_path;
    }
    if (swf.empty()) {
        if (!game->data || game->size == 0 ||
            game->size > static_cast<std::size_t>(std::numeric_limits<std::streamsize>::max())) {
            log_message(RETRO_LOG_ERROR, "Winebox needs an accessible SWF path or SWF bytes from the frontend.");
            return false;
        }
        std::string swf_leaf = game->path
            ? std::filesystem::u8path(game->path).filename().string() : "content.swf";
        if (!safe_leaf_name(swf_leaf) || std::filesystem::u8path(swf_leaf).extension() != ".swf")
            swf_leaf = "content.swf";
        swf = save_root / swf_leaf;
        std::ofstream output(swf, std::ios::binary | std::ios::trunc);
        if (!output) {
            log_message(RETRO_LOG_ERROR, ("Winebox could not create a cached SWF in its save directory: " + swf.string()).c_str());
            return false;
        }
        output.write(static_cast<const char*>(game->data), static_cast<std::streamsize>(game->size));
        output.close();
        if (!output) {
            log_message(RETRO_LOG_ERROR, ("Winebox could not write the cached SWF: " + swf.string()).c_str());
            return false;
        }
    }
    log_message(RETRO_LOG_INFO, ("Winebox starting; root=" + root_archive.string() +
                                 "; projector=" + projector.string() + "; swf=" + swf.string()).c_str());

    const std::string projector_name = projector.filename().string();
    const std::string swf_name = swf.filename().string();
    if (!safe_leaf_name(projector_name) || !safe_leaf_name(swf_name))
        return false;

    g_save_dir = save_root.string();
    const auto projector_parent = projector.parent_path().string();
    const auto swf_parent = swf.parent_path().string();
    const std::string guest_projector = "d:\\" + projector_name;
    const std::string guest_swf = "e:\\" + swf_name;

    // Preserve the root image's C: Wine prefix (including C:\\windows).
    // Mount user-supplied files on D: and E: instead of shadowing that prefix.
    g_guest_args = {"winebox", "-root", g_save_dir, "-zip", root_archive.string(), "-novideo",
                    "-mount_drive", projector_parent, "d",
                    "-mount_drive", swf_parent, "e",
                    "/bin/wine", guest_projector, guest_swf};
    return true;
}

void start_guest() {
    if (g_guest.joinable())
        g_guest.join();
    const std::vector<std::string> args = g_guest_args;
    g_exit_reported = false;
    g_guest_finished.store(false, std::memory_order_release);
    g_guest = std::thread([args] {
        std::vector<const char*> argv;
        argv.reserve(args.size());
        for (const auto& arg : args)
            argv.push_back(arg.c_str());
        const int result = boxedmain(static_cast<int>(argv.size()), argv.data());
        g_guest_exit_code = result;
        g_guest_finished.store(true, std::memory_order_release);
    });
}

void stop_guest() {
    if (!g_guest.joinable())
        return;
    // SDL_QUIT can be posted before SDL has initialized its event queue. Keep
    // retrying until the startup thread has reached the main loop and exits.
    while (!g_guest_finished.load(std::memory_order_acquire)) {
        winebox_boxedwine_request_quit();
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    g_guest.join();
}

void RETRO_CALLCONV keyboard_event(bool down,
                                   unsigned keycode,
                                   std::uint32_t character,
                                   std::uint16_t modifiers) {
    if (g_input)
        g_input->keyboard_event(down, keycode, character, modifiers);
}

void render_input_overlay(void* context, std::uint32_t* pixels, unsigned width, unsigned height) {
    if (context)
        static_cast<winebox::SdlInputAdapter*>(context)->render_overlay(pixels, width, height);
}

} // namespace

namespace winebox {
RetroHost* current_host() noexcept {
    return g_host.get();
}
} // namespace winebox

extern "C" {

RETRO_API unsigned RETRO_CALLCONV retro_api_version(void) {
    return RETRO_API_VERSION;
}

RETRO_API void RETRO_CALLCONV retro_set_environment(retro_environment_t callback) {
    g_environment = callback;
    if (!callback)
        return;
    retro_log_callback log_callback{};
    if (callback(RETRO_ENVIRONMENT_GET_LOG_INTERFACE, &log_callback))
        g_log = log_callback.log;

    static const retro_variable variables[] = {
        {"winebox_root", "Winebox root archive; TinyCore15Wine11.0.zip"},
        {"winebox_projector", "Flash projector executable; FlashPlayer.exe"},
        {"winebox_controls", "Controller preset; Generic Keyboard|Mouse Left Stick|Mouse Right Stick|Custom Mapper"},
        {nullptr, nullptr}
    };
    callback(RETRO_ENVIRONMENT_SET_VARIABLES, const_cast<retro_variable*>(variables));

    static const retro_system_content_info_override content_overrides[] = {
        {"swf", false, false},
        {nullptr, false, false}
    };
    callback(RETRO_ENVIRONMENT_SET_CONTENT_INFO_OVERRIDE,
             const_cast<retro_system_content_info_override*>(content_overrides));

    static const retro_controller_description controller_types[] = {
        {"RetroPad", RETRO_DEVICE_JOYPAD},
        {"RetroPad with Analog", RETRO_DEVICE_ANALOG}
    };
    static const retro_controller_info controller_info[] = {
        {controller_types, 2},
        {nullptr, 0}
    };
    callback(RETRO_ENVIRONMENT_SET_CONTROLLER_INFO, const_cast<retro_controller_info*>(controller_info));

    static const retro_input_descriptor input_descriptors[] = {
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_B, "B"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_Y, "Y"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_SELECT, "Select"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_START, "Start"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_UP, "D-pad Up"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_DOWN, "D-pad Down"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_LEFT, "D-pad Left"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_RIGHT, "D-pad Right"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_A, "A"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_X, "X"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_L, "L"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_R, "R"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_L2, "L2"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_R2, "R2"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_L3, "L3 (Controls Menu)"},
        {0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_R3, "R3"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_ESCAPE, "Keyboard Escape"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_RETURN, "Keyboard Enter"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_SPACE, "Keyboard Space"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_LCTRL, "Keyboard Left Control"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_LSHIFT, "Keyboard Left Shift"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_LALT, "Keyboard Left Alt"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_UP, "Keyboard Up"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_DOWN, "Keyboard Down"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_LEFT, "Keyboard Left"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_RIGHT, "Keyboard Right"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_a, "Keyboard A"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_d, "Keyboard D"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_s, "Keyboard S"},
        {0, RETRO_DEVICE_KEYBOARD, 0, RETROK_w, "Keyboard W"},
        {0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_ID_ANALOG_X, "Left Stick X"},
        {0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_ID_ANALOG_Y, "Left Stick Y"},
        {0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_RIGHT, RETRO_DEVICE_ID_ANALOG_X, "Right Stick X"},
        {0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_RIGHT, RETRO_DEVICE_ID_ANALOG_Y, "Right Stick Y"},
        {0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_X, "Mouse X"},
        {0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_Y, "Mouse Y"},
        {0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_LEFT, "Mouse Left"},
        {0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_RIGHT, "Mouse Right"},
        {0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_MIDDLE, "Mouse Middle"},
        {0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_WHEELUP, "Mouse Wheel Up"},
        {0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_WHEELDOWN, "Mouse Wheel Down"},
        {0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_X, "Pointer X"},
        {0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_Y, "Pointer Y"},
        {0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_PRESSED, "Pointer Press"},
        {0, RETRO_DEVICE_NONE, 0, 0, nullptr}
    };
    callback(RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS, const_cast<retro_input_descriptor*>(input_descriptors));
}

RETRO_API void RETRO_CALLCONV retro_set_video_refresh(retro_video_refresh_t callback) {
    g_video = callback;
    if (g_host)
        g_host->set_video_callback(callback);
}
RETRO_API void RETRO_CALLCONV retro_set_audio_sample(retro_audio_sample_t callback) {
    g_audio_sample = callback;
    if (g_host && !g_audio_batch)
        g_host->set_audio_callback(single_sample_batch);
}
RETRO_API void RETRO_CALLCONV retro_set_audio_sample_batch(retro_audio_sample_batch_t callback) {
    g_audio_batch = callback;
    if (g_host)
        g_host->set_audio_callback(g_audio_batch ? g_audio_batch : single_sample_batch);
}
RETRO_API void RETRO_CALLCONV retro_set_input_poll(retro_input_poll_t callback) {
    g_input_poll = callback;
    if (g_host)
        g_host->set_input_callbacks(g_input_poll, g_input_state);
    if (g_input)
        g_input->set_callbacks(g_input_poll, g_input_state);
}
RETRO_API void RETRO_CALLCONV retro_set_input_state(retro_input_state_t callback) {
    g_input_state = callback;
    if (g_host)
        g_host->set_input_callbacks(g_input_poll, g_input_state);
    if (g_input)
        g_input->set_callbacks(g_input_poll, g_input_state);
}
RETRO_API void RETRO_CALLCONV retro_set_controller_port_device(unsigned, unsigned) {}

RETRO_API void RETRO_CALLCONV retro_init(void) {
    if (g_initialized)
        return;
    if (g_environment) {
        enum retro_pixel_format format = RETRO_PIXEL_FORMAT_XRGB8888;
        g_pixel_format_supported = g_environment(RETRO_ENVIRONMENT_SET_PIXEL_FORMAT, &format);
        if (!g_pixel_format_supported)
            log_message(RETRO_LOG_ERROR, "Winebox requires frontend support for XRGB8888 video.");
    }
    const auto audio_batch = g_audio_batch ? g_audio_batch : single_sample_batch;
    g_host = std::make_unique<winebox::RetroHost>(g_video, audio_batch, g_input_poll, g_input_state);
    std::vector<std::uint32_t> blank(static_cast<std::size_t>(kInitialWidth) * kInitialHeight, 0);
    g_host->publish_video(blank.data(), kInitialWidth, kInitialHeight,
                          static_cast<std::size_t>(kInitialWidth) * sizeof(std::uint32_t));
    g_video_stats_started = std::chrono::steady_clock::now();
    g_video_stats_publications = g_host->video_publication_count();
    g_video_stats_publish_nanoseconds = g_host->video_publication_time_nanoseconds();
    g_video_stats_present_calls = 0;
    g_video_stats_present_milliseconds = 0.0;
    g_input = std::make_unique<winebox::SdlInputAdapter>(g_input_poll, g_input_state,
                                                         kInitialWidth, kInitialHeight);
    if (g_environment) {
        static retro_keyboard_callback keyboard_callback{keyboard_event};
        const bool keyboard_callback_supported =
            g_environment(RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK, &keyboard_callback);
        g_input->set_keyboard_callback_enabled(keyboard_callback_supported);
        log_message(keyboard_callback_supported ? RETRO_LOG_INFO : RETRO_LOG_WARN,
                    keyboard_callback_supported ? "Winebox registered the frontend keyboard callback."
                                               : "Winebox keyboard callback unavailable; using input polling fallback.");
    }
    g_initialized = true;
}

RETRO_API void RETRO_CALLCONV retro_deinit(void) {
    retro_unload_game();
    g_input.reset();
    g_host.reset();
    g_initialized = false;
}

RETRO_API void RETRO_CALLCONV retro_get_system_info(struct retro_system_info* info) {
    if (!info)
        return;
    info->library_name = "Winebox";
    info->library_version = "0.1";
    info->valid_extensions = "swf";
    info->need_fullpath = false;
    info->block_extract = true;
}

RETRO_API void RETRO_CALLCONV retro_get_system_av_info(struct retro_system_av_info* info) {
    if (!info)
        return;
    info->geometry = {kInitialWidth, kInitialHeight, 4096, 4096, 4.0f / 3.0f};
    info->timing = {60.0, 44100.0};
}

RETRO_API void RETRO_CALLCONV retro_reset(void) {
    if (!g_game_loaded)
        return;
    stop_guest();
    if (g_input)
        g_input->reset();
    start_guest();
}

RETRO_API void RETRO_CALLCONV retro_run(void) {
    if (!g_initialized || !g_host)
        return;
    if (g_game_loaded && g_guest_finished.load(std::memory_order_acquire) && !g_exit_reported) {
        log_message(g_guest_exit_code == 0 ? RETRO_LOG_INFO : RETRO_LOG_WARN,
                    g_guest_exit_code == 0 ? "BoxedWine guest exited normally." : "BoxedWine guest exited with an error.");
        g_exit_reported = true;
    }
    if (g_input) {
        std::string controls;
        if (get_variable("winebox_controls", controls)) {
            using Mode = winebox::SdlInputAdapter::ControllerMode;
            const Mode mode = controls == "Mouse Left Stick" ? Mode::MouseLeftStick
                : controls == "Mouse Right Stick" ? Mode::MouseRightStick
                : controls == "Custom Mapper" ? Mode::CustomMapper
                : Mode::GenericKeyboard;
            g_input->set_controller_mode(mode);
        }
        g_input->poll_and_enqueue();
    }
    const auto present_started = std::chrono::steady_clock::now();
    g_host->present_video(g_input ? render_input_overlay : nullptr, g_input.get(),
                          g_input && g_input->overlay_active());
    g_video_stats_present_milliseconds +=
        std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - present_started).count();
    ++g_video_stats_present_calls;

    unsigned width = 0, height = 0;
    if (g_host->video_dimensions(width, height) && g_input)
        g_input->set_dimensions(width, height);

    const auto now = std::chrono::steady_clock::now();
    const auto elapsed = now - g_video_stats_started;
    if (g_game_loaded && elapsed >= kVideoStatsInterval) {
        const double seconds = std::chrono::duration<double>(elapsed).count();
        const auto publications = g_host->video_publication_count();
        const auto publish_nanoseconds = g_host->video_publication_time_nanoseconds();
        const auto publication_delta = publications - g_video_stats_publications;
        const double submitted_fps = static_cast<double>(publication_delta) / seconds;
        const double frontend_fps = static_cast<double>(g_video_stats_present_calls) / seconds;
        const double average_publish_ms = publication_delta
            ? static_cast<double>(publish_nanoseconds - g_video_stats_publish_nanoseconds) /
                  1'000'000.0 / static_cast<double>(publication_delta)
            : 0.0;
        const double average_present_ms = g_video_stats_present_calls
            ? g_video_stats_present_milliseconds / static_cast<double>(g_video_stats_present_calls)
            : 0.0;
        unsigned frame_width = 0, frame_height = 0;
        g_host->video_dimensions(frame_width, frame_height);
        const auto non_black_pixels = g_host->video_non_black_pixel_count();
        const auto total_pixels = static_cast<std::uint64_t>(frame_width) * frame_height;
        char report[256]{};
        std::snprintf(report, sizeof(report),
                      "Winebox software video: submissions %.1f/s, presents %.1f/s, nonblack %llu/%llu px at %ux%u, avg copy %.3f ms, avg present %.3f ms",
                      submitted_fps, frontend_fps,
                      static_cast<unsigned long long>(non_black_pixels),
                      static_cast<unsigned long long>(total_pixels), frame_width, frame_height,
                      average_publish_ms, average_present_ms);
        log_message(RETRO_LOG_INFO, report);
        g_video_stats_started = now;
        g_video_stats_publications = publications;
        g_video_stats_publish_nanoseconds = publish_nanoseconds;
        g_video_stats_present_calls = 0;
        g_video_stats_present_milliseconds = 0.0;
    }
    g_host->drain_audio_exact(kAudioFramesPerRun);
}

RETRO_API size_t RETRO_CALLCONV retro_serialize_size(void) { return 0; }
RETRO_API bool RETRO_CALLCONV retro_serialize(void*, size_t) { return false; }
RETRO_API bool RETRO_CALLCONV retro_unserialize(const void*, size_t) { return false; }
RETRO_API void RETRO_CALLCONV retro_cheat_reset(void) {}
RETRO_API void RETRO_CALLCONV retro_cheat_set(unsigned, bool, const char*) {}

RETRO_API bool RETRO_CALLCONV retro_load_game(const struct retro_game_info* game) {
    if (!g_initialized)
        retro_init();
    if (g_game_loaded || !g_pixel_format_supported || !prepare_launch(game))
        return false;
    if (g_input)
        g_input->set_config_file(mapping_file_for_content(game->path));
    g_game_loaded = true;
    start_guest();
    return true;
}

RETRO_API bool RETRO_CALLCONV retro_load_game_special(unsigned, const struct retro_game_info*, size_t) { return false; }

RETRO_API void RETRO_CALLCONV retro_unload_game(void) {
    if (g_game_loaded) {
        stop_guest();
        g_game_loaded = false;
        g_guest_args.clear();
        if (g_input)
            g_input->reset();
    }
}

RETRO_API unsigned RETRO_CALLCONV retro_get_region(void) { return RETRO_REGION_NTSC; }
RETRO_API void* RETRO_CALLCONV retro_get_memory_data(unsigned) { return nullptr; }
RETRO_API size_t RETRO_CALLCONV retro_get_memory_size(unsigned) { return 0; }
RETRO_API void RETRO_CALLCONV retro_set_audio_callback(const struct retro_audio_callback*) {}
RETRO_API void RETRO_CALLCONV retro_set_keyboard_callback(const struct retro_keyboard_callback*) {}
RETRO_API void RETRO_CALLCONV retro_set_frame_time_callback(const struct retro_frame_time_callback*) {}
RETRO_API void RETRO_CALLCONV retro_set_hw_render(const struct retro_hw_render_callback*) {}

} // extern "C"
