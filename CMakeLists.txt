cmake_minimum_required(VERSION 3.22)

project(
    Flow8PcController
    VERSION 0.1.0
    DESCRIPTION "Cross-platform PC controller for the Behringer FLOW 8"
    LANGUAGES CXX
)

option(FLOW8_BUILD_TESTS "Build FLOW 8 PC Controller tests" ON)
option(
    FLOW8_REQUIRE_BLUETOOTH
    "Fail configuration when the Qt Bluetooth development component is unavailable"
    OFF
)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_AUTOMOC ON)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")
include(CompilerWarnings)
include(Platform)

find_package(Qt6 6.2 REQUIRED COMPONENTS Core Widgets)
find_package(Qt6 6.2 QUIET COMPONENTS Bluetooth)

if(TARGET Qt6::Bluetooth)
    set(FLOW8_HAS_QT_BLUETOOTH ON)
    message(STATUS "Qt Bluetooth is available")
elseif(FLOW8_REQUIRE_BLUETOOTH)
    message(FATAL_ERROR
        "Qt Bluetooth is required but was not found. Install the Qt Connectivity "
        "development component or set CMAKE_PREFIX_PATH to a complete Qt 6 installation."
    )
else()
    set(FLOW8_HAS_QT_BLUETOOTH OFF)
    message(WARNING
        "Qt Bluetooth was not found. Protocol, simulator, tests, and GUI will build, but "
        "BLE transport and tools are omitted until Qt Connectivity is installed."
    )
endif()

add_library(flow8_protocol STATIC
    src/protocol/codec.cpp
    src/protocol/codec.h
    src/protocol/flow8_protocol.cpp
    src/protocol/flow8_protocol.h
    src/protocol/packet.cpp
    src/protocol/packet.h
    src/protocol/sysex.cpp
    src/protocol/sysex.h
)
add_library(flow8::protocol ALIAS flow8_protocol)

target_include_directories(flow8_protocol
    PUBLIC
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>"
)
target_link_libraries(flow8_protocol PUBLIC Qt6::Core)
target_compile_features(flow8_protocol PUBLIC cxx_std_20)
flow8_enable_warnings(flow8_protocol)
flow8_configure_platform(flow8_protocol)

add_library(flow8_core STATIC
    src/core/flow8_device.cpp
    src/core/flow8_device.h
    src/core/flow8_state.cpp
    src/core/flow8_state.h
    src/core/flow8_transport.cpp
    src/core/flow8_transport.h
)
add_library(flow8::core ALIAS flow8_core)

target_include_directories(flow8_core
    PUBLIC
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>"
)
target_link_libraries(flow8_core PUBLIC flow8::protocol Qt6::Core)
target_compile_features(flow8_core PUBLIC cxx_std_20)
flow8_enable_warnings(flow8_core)
flow8_configure_platform(flow8_core)

add_library(flow8_simulator STATIC
    src/simulator/fake_transport.cpp
    src/simulator/fake_transport.h
)
add_library(flow8::simulator ALIAS flow8_simulator)
target_include_directories(flow8_simulator
    PUBLIC
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>"
)
target_link_libraries(flow8_simulator PUBLIC flow8::core flow8::protocol Qt6::Core)
flow8_enable_warnings(flow8_simulator)
flow8_configure_platform(flow8_simulator)

add_executable(flow8-packet-decoder tools/packet_decoder/main.cpp)
target_link_libraries(flow8-packet-decoder PRIVATE flow8::protocol Qt6::Core)
flow8_enable_warnings(flow8-packet-decoder)
flow8_configure_platform(flow8-packet-decoder)

add_executable(flow8-state-dump-decoder tools/state_dump_decoder/main.cpp)
target_link_libraries(flow8-state-dump-decoder PRIVATE flow8::protocol Qt6::Core)
flow8_enable_warnings(flow8-state-dump-decoder)
flow8_configure_platform(flow8-state-dump-decoder)

add_library(flow8_ui STATIC
    src/ui/connection_bar.cpp
    src/ui/connection_bar.h
    src/ui/main_window.cpp
    src/ui/main_window.h
    src/ui/mixer/channel_strip.cpp
    src/ui/mixer/channel_strip.h
    src/ui/mixer/main_strip.cpp
    src/ui/mixer/main_strip.h
    src/ui/mixer/mixer_widget.cpp
    src/ui/mixer/mixer_widget.h
)
add_library(flow8::ui ALIAS flow8_ui)
target_include_directories(flow8_ui
    PUBLIC
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>"
)
target_link_libraries(flow8_ui PUBLIC flow8::core Qt6::Widgets)
flow8_enable_warnings(flow8_ui)
flow8_configure_platform(flow8_ui)

if(FLOW8_HAS_QT_BLUETOOTH)
    add_library(flow8_ble STATIC
        src/ble/ble_device.cpp
        src/ble/ble_device.h
        src/ble/ble_services.cpp
        src/ble/ble_services.h
        src/ble/ble_transport.cpp
        src/ble/ble_transport.h
    )
    add_library(flow8::ble ALIAS flow8_ble)
    target_include_directories(flow8_ble
        PUBLIC
            "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>"
    )
    target_link_libraries(flow8_ble PUBLIC flow8::core Qt6::Bluetooth Qt6::Core)
    flow8_enable_warnings(flow8_ble)
    flow8_configure_platform(flow8_ble)

    add_executable(flow8-ble-scanner tools/ble_scanner/main.cpp)
    target_link_libraries(flow8-ble-scanner PRIVATE flow8::ble Qt6::Core)
    flow8_enable_warnings(flow8-ble-scanner)
    flow8_configure_platform(flow8-ble-scanner)

    add_executable(flow8-ble-monitor tools/ble_monitor/main.cpp)
    target_link_libraries(flow8-ble-monitor PRIVATE flow8::ble flow8::protocol Qt6::Core)
    flow8_enable_warnings(flow8-ble-monitor)
    flow8_configure_platform(flow8-ble-monitor)
endif()

add_executable(flow8-controller src/app/main.cpp)
target_link_libraries(flow8-controller PRIVATE flow8::core flow8::simulator flow8::ui Qt6::Widgets)
target_compile_definitions(flow8-controller PRIVATE
    FLOW8_APP_VERSION="${PROJECT_VERSION}"
)
flow8_enable_warnings(flow8-controller)
flow8_configure_platform(flow8-controller)

include(CTest)
if(BUILD_TESTING AND FLOW8_BUILD_TESTS)
    find_package(Qt6 6.2 REQUIRED COMPONENTS Test)
    add_subdirectory(tests)
endif()

message(STATUS "FLOW 8 PC Controller ${PROJECT_VERSION}")
message(STATUS "Host platform: ${CMAKE_HOST_SYSTEM_NAME}")
message(STATUS "Target platform: ${CMAKE_SYSTEM_NAME}")
message(STATUS "Qt version: ${Qt6_VERSION}")
message(STATUS "Qt Bluetooth available: ${FLOW8_HAS_QT_BLUETOOTH}")
