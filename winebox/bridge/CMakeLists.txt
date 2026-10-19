cmake_minimum_required(VERSION 3.20)
project(winebox_retro_host LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

enable_testing()

add_library(winebox_retro_host STATIC retro_host.cpp audio_adapter.cpp)
target_include_directories(winebox_retro_host
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}
        ${CMAKE_CURRENT_SOURCE_DIR}/../../docs/reference
)

add_executable(winebox_retro_host_test retro_host_test.cpp)
target_link_libraries(winebox_retro_host_test PRIVATE winebox_retro_host)
add_test(NAME winebox_retro_host_test COMMAND winebox_retro_host_test)

add_executable(winebox_audio_adapter_test audio_adapter_test.cpp)
target_link_libraries(winebox_audio_adapter_test PRIVATE winebox_retro_host)
add_test(NAME winebox_audio_adapter_test COMMAND winebox_audio_adapter_test)
