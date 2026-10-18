cmake_minimum_required(VERSION 3.24)
project(CrossWarpIddFrameBufferTests LANGUAGES CXX)

add_executable(idd-frame-buffer-contract-test frame_buffer_contract_test.cpp)
target_compile_features(idd-frame-buffer-contract-test PRIVATE cxx_std_20)
target_compile_definitions(idd-frame-buffer-contract-test PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_include_directories(idd-frame-buffer-contract-test PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/..)

enable_testing()
add_test(NAME idd-frame-buffer-contract COMMAND idd-frame-buffer-contract-test)
