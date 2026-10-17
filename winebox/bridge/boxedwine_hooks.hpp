#pragma once

namespace winebox {
class RetroHost;

// Implemented by the libretro module. BoxedWine's screen/audio/input adapters
// may publish into the current host from emulator threads; frontend callbacks
// remain confined to retro_run().
RetroHost* current_host() noexcept;
}
