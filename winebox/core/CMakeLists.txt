cmake_minimum_required(VERSION 3.20)
project(winebox_libretro_core_objects LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(WINEBOX_SDL2_INCLUDE_DIR "" CACHE PATH "Path to the pinned BoxedWine SDL2 include directory")
if(NOT EXISTS "${WINEBOX_SDL2_INCLUDE_DIR}/SDL.h")
    message(FATAL_ERROR "Set WINEBOX_SDL2_INCLUDE_DIR to BoxedWine's lib/sdl2/include directory")
endif()

add_library(winebox_libretro_core_objects OBJECT
    libretro_core.cpp
    ../bridge/retro_host.cpp
    ../bridge/audio_adapter.cpp
    ../bridge/input_sdl_adapter.cpp
)
target_include_directories(winebox_libretro_core_objects PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/../bridge
    ${CMAKE_CURRENT_SOURCE_DIR}/../../docs/reference
    ${WINEBOX_SDL2_INCLUDE_DIR}
)
target_compile_definitions(winebox_libretro_core_objects PRIVATE BOXEDWINE_LIBRETRO=1)
if(MSVC)
    target_compile_options(winebox_libretro_core_objects PRIVATE /W4)
else()
    target_compile_options(winebox_libretro_core_objects PRIVATE -Wall -Wextra -Werror)
endif()
