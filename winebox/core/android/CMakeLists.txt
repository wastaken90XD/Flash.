cmake_minimum_required(VERSION 3.22)
project(winebox_libretro_android LANGUAGES C CXX)

if(NOT ANDROID)
    message(FATAL_ERROR "Winebox Android must be configured with the Android NDK toolchain")
endif()
if(NOT DEFINED BOXEDWINE_ROOT OR NOT EXISTS "${BOXEDWINE_ROOT}/source/sdl/main.cpp")
    message(FATAL_ERROR "Set BOXEDWINE_ROOT to the pinned BoxedWine checkout")
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# Libretro owns the screen, audio device, and controller polling. SDL remains
# embedded for BoxedWine's event queue and virtual joystick API; its native
# Android display/audio backends are not initialized by the core.
set(SDL_SHARED OFF CACHE BOOL "" FORCE)
set(SDL_STATIC ON CACHE BOOL "" FORCE)
set(SDL_STATIC_PIC ON CACHE BOOL "" FORCE)
set(SDL_TEST OFF CACHE BOOL "" FORCE)
set(SDL_TESTS OFF CACHE BOOL "" FORCE)
set(SDL_INSTALL OFF CACHE BOOL "" FORCE)
set(SDL_AUDIO ON CACHE BOOL "" FORCE)
set(SDL_VIDEO ON CACHE BOOL "" FORCE)
set(SDL_RENDER OFF CACHE BOOL "" FORCE)
set(VIDEO_OPENGL OFF CACHE BOOL "" FORCE)
set(VIDEO_OPENGLES OFF CACHE BOOL "" FORCE)
set(VIDEO_VULKAN OFF CACHE BOOL "" FORCE)
set(SDL_JOYSTICK ON CACHE BOOL "" FORCE)
set(SDL_HAPTIC OFF CACHE BOOL "" FORCE)
set(HIDAPI OFF CACHE BOOL "" FORCE)
set(SDL_POWER OFF CACHE BOOL "" FORCE)
set(SDL_SENSOR OFF CACHE BOOL "" FORCE)
add_subdirectory("${BOXEDWINE_ROOT}/lib/sdl2" "${CMAKE_BINARY_DIR}/sdl2" EXCLUDE_FROM_ALL)
target_compile_definitions(SDL2-static PRIVATE BOXEDWINE_LIBRETRO=1)

file(GLOB_RECURSE BW_CPP CONFIGURE_DEPENDS "${BOXEDWINE_ROOT}/source/*.cpp")
file(GLOB BW_LINUX CONFIGURE_DEPENDS "${BOXEDWINE_ROOT}/platform/linux/*.cpp")
file(GLOB BW_SDL CONFIGURE_DEPENDS "${BOXEDWINE_ROOT}/platform/sdl/*.cpp")
file(GLOB_RECURSE BW_SOFTFLOAT CONFIGURE_DEPENDS "${BOXEDWINE_ROOT}/lib/softfloat/source/*.c")
set(BW_MINIZIP
    "${BOXEDWINE_ROOT}/lib/zlib/contrib/minizip/ioapi.c"
    "${BOXEDWINE_ROOT}/lib/zlib/contrib/minizip/mztools.c"
    "${BOXEDWINE_ROOT}/lib/zlib/contrib/minizip/unzip.c"
    "${BOXEDWINE_ROOT}/lib/zlib/contrib/minizip/zip.c")

# The libretro build launches one guest application directly. It does not ship
# BoxedWine's desktop launcher, native OpenGL presenter, or network installer.
list(FILTER BW_CPP EXCLUDE REGEX "/source/test/")
list(FILTER BW_CPP EXCLUDE REGEX "/source/ui/")
list(FILTER BW_CPP EXCLUDE REGEX "/source/opengl/sdl/")
list(FILTER BW_CPP EXCLUDE REGEX "/source/util/networkutils\\.cpp$")
list(FILTER BW_LINUX EXCLUDE REGEX "/platformOpenGL\\.cpp$")
list(FILTER BW_SDL EXCLUDE REGEX "/knativescreenGL\\.cpp$")

set(WINEBOX_SOURCES
    ${BW_CPP}
    ${BW_LINUX}
    ${BW_SDL}
    ${BW_SOFTFLOAT}
    ${BW_MINIZIP}
    "${CMAKE_CURRENT_LIST_DIR}/../libretro_core.cpp"
    "${CMAKE_CURRENT_LIST_DIR}/../../bridge/retro_host.cpp"
    "${CMAKE_CURRENT_LIST_DIR}/../../bridge/audio_adapter.cpp"
    "${CMAKE_CURRENT_LIST_DIR}/../../bridge/input_sdl_adapter.cpp")

if(ANDROID_ABI STREQUAL "arm64-v8a")
    file(GLOB_RECURSE BW_ASMJIT CONFIGURE_DEPENDS "${BOXEDWINE_ROOT}/lib/asmjit/asmjit/*.cpp")
    list(APPEND WINEBOX_SOURCES ${BW_ASMJIT})
endif()

add_library(winebox_libretro SHARED ${WINEBOX_SOURCES})
target_include_directories(winebox_libretro PRIVATE
    "${CMAKE_CURRENT_LIST_DIR}/../../bridge"
    "${CMAKE_CURRENT_LIST_DIR}/../../../docs/reference"
    "${BOXEDWINE_ROOT}/include"
    "${BOXEDWINE_ROOT}/lib/simde"
    "${BOXEDWINE_ROOT}/lib/asmjit"
    "${BOXEDWINE_ROOT}/lib/zlib"
    "${BOXEDWINE_ROOT}/lib/zlib/contrib/minizip"
    "${BOXEDWINE_ROOT}/lib/softfloat/source/include"
    "${BOXEDWINE_ROOT}/lib/softfloat/source"
    "${BOXEDWINE_ROOT}/lib/softfloat/source/8086-SSE")
target_compile_definitions(winebox_libretro PRIVATE
    BOXEDWINE_LIBRETRO=1
    BOXEDWINE_DISABLE_UI=1
    BOXEDWINE_POSIX=1
    BOXEDWINE_ZLIB=1
    BOXEDWINE_HAS_SETJMP=1
    BOXEDWINE_MULTI_THREADED=1
    SDL2=1
    SDL_MAIN_HANDLED=1
    SIMDE_SSE2_NO_NATIVE=1)
if(ANDROID_ABI STREQUAL "arm64-v8a")
    target_compile_definitions(winebox_libretro PRIVATE
        ASMJIT_STATIC=1
        BOXEDWINE_64=1
        BOXEDWINE_JIT=1
        BOXEDWINE_JIT_ARMV8=1
        BOXEDWINE_MEM_CACHE=1
        BOXEDWINE_HOST_EXCEPTIONS=1
        SIMDE_NO_NEON=1
        SIMDE_SSE2_NO_NEON=1)
else()
    # Android 5 ARMv7 is interpreter-only: upstream BoxedWine has no ARMv7 JIT.
    target_compile_definitions(winebox_libretro PRIVATE
        ASMJIT_NO_AARCH64=1
        ASMJIT_NO_X86=1)
endif()
target_link_libraries(winebox_libretro PRIVATE SDL2-static z log android dl m)
if(ANDROID_ABI STREQUAL "armeabi-v7a")
    target_link_libraries(winebox_libretro PRIVATE atomic)
endif()
target_link_options(winebox_libretro PRIVATE
    "-Wl,-z,max-page-size=16384"
    "-Wl,--no-undefined")
set_target_properties(winebox_libretro PROPERTIES
    OUTPUT_NAME winebox_libretro
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib/${ANDROID_ABI}"
    CXX_VISIBILITY_PRESET default
    C_VISIBILITY_PRESET default)
