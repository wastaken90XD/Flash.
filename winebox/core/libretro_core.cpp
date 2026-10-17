#include "../bridge/audio_adapter.hpp"
#include "../bridge/boxedwine_hooks.hpp"
#include "../bridge/input_sdl_adapter.hpp"
#include "../bridge/retro_host.hpp"

#include <SDL.h>
#include <libretro.h>

#include <atomic>
#include <chrono>
#include <cstdio>
#include <filesystem>
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

bool prepare_launch(const retro_game_info* game) {
    if (!game || !game->path || !g_environment)
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
    const std::filesystem::path swf = std::filesystem::u8path(game->path);
    if (!regular_file(root_archive)) {
        log_message(RETRO_LOG_ERROR, ("Winebox root archive is missing or not a regular file: " + root_archive.string()).c_str());
        return false;
    }
    if (!regular_file(projector)) {
        log_message(RETRO_LOG_ERROR, ("Winebox Flash projector is missing or not a regular file: " + projector.string()).c_str());
        return false;
    }
    if (!regular_file(swf)) {
        log_message(RETRO_LOG_ERROR, ("Winebox SWF is missing or not a regular file: " + swf.string()).c_str());
        return false;
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
        {nullptr, nullptr}
    };
    callback(RETRO_ENVIRONMENT_SET_VARIABLES, const_cast<retro_variable*>(variables));
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
    info->need_fullpath = true;
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
    if (g_input)
        g_input->poll_and_enqueue();
    const auto present_started = std::chrono::steady_clock::now();
    g_host->present_video();
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
