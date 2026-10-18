cmake_minimum_required(VERSION 3.24)
project(CrossWarpIddClient LANGUAGES CXX)

add_library(crosswarp-idd-client STATIC idd_client.cpp)
target_compile_features(crosswarp-idd-client PUBLIC cxx_std_20)
target_compile_definitions(crosswarp-idd-client PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_include_directories(crosswarp-idd-client PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_link_libraries(crosswarp-idd-client PUBLIC setupapi)

add_executable(idd-client-contract-test contract_test.cpp idd_client.cpp)
target_compile_features(idd-client-contract-test PRIVATE cxx_std_20)
target_compile_definitions(idd-client-contract-test PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_include_directories(idd-client-contract-test PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
target_link_libraries(idd-client-contract-test PRIVATE setupapi)

enable_testing()
add_test(NAME idd-client-contract COMMAND idd-client-contract-test)
