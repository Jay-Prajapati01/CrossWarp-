cmake_minimum_required(VERSION 3.24)
project(UnifiedWindowCaptureSmoke LANGUAGES CXX)

add_executable(window-capture-smoke capture_smoke.cpp)
target_compile_features(window-capture-smoke PRIVATE cxx_std_20)
target_compile_definitions(window-capture-smoke PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_link_libraries(window-capture-smoke PRIVATE d3d11 dxgi windowsapp)
