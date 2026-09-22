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
option(
    FLOW8_REQUIRE_MINGW
    "Fail configuration unless the Windows target uses the GNU MinGW-w64 toolchain"
    OFF
)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_AUTOMOC ON)

if(FLOW8_REQUIRE_MINGW)
    if(NOT WIN32)
        message(FATAL_ERROR "FLOW8_REQUIRE_MINGW is only valid for a Windows target.")
    endif()
    if(NOT MINGW OR NOT CMAKE_CXX_COMPILER_ID STREQUAL "GNU")
        message(FATAL_ERROR
            "The selected preset requires Qt MinGW-w64. Put the Qt MinGW bin directory "
            "containing g++.exe first on PATH; MSVC is not required."
        )
    endif()
endif()

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")
include(CompilerWarnings)
include(Platform)

find_package(Qt6 6.2 REQUIRED COMPONENTS Core Widgets)
if(FLOW8_REQUIRE_BLUETOOTH)
    find_package(Qt6 6.2 REQUIRED COMPONENTS Bluetooth)
else()
    find_package(Qt6 6.2 QUIET COMPONENTS Bluetooth)
endif()

if(FLOW8_REQUIRE_MINGW)
    if(NOT TARGET Qt6::qmake)
        message(FATAL_ERROR
            "Qt6::qmake is required to verify that the selected Qt kit is MinGW-w64."
        )
    endif()
    get_target_property(_flow8_mingw_qmake Qt6::qmake IMPORTED_LOCATION)
    execute_process(
        COMMAND "${_flow8_mingw_qmake}" -query QMAKE_XSPEC
        RESULT_VARIABLE _flow8_mingw_qmake_result
        OUTPUT_VARIABLE _flow8_qmake_xspec
        OUTPUT_STRIP_TRAILING_WHITESPACE
    )
    if(NOT _flow8_mingw_qmake_result EQUAL 0
       OR NOT _flow8_qmake_xspec MATCHES "^win32-g\\+\\+")
        message(FATAL_ERROR
            "The discovered Qt installation is not a Qt MinGW-w64 kit "
            "(QMAKE_XSPEC=${_flow8_qmake_xspec}). Set CMAKE_PREFIX_PATH or Qt6_DIR "
            "to the Qt mingw_64 installation."
        )
    endif()
endif()

set(FLOW8_LRELEASE_EXECUTABLE "" CACHE FILEPATH "Path to the Qt 6 lrelease executable")
if(NOT FLOW8_LRELEASE_EXECUTABLE)
    set(_flow8_qt_bin_hint "")
    if(TARGET Qt6::qmake)
        get_target_property(_flow8_qmake_executable Qt6::qmake IMPORTED_LOCATION)
        get_filename_component(_flow8_qt_bin_hint "${_flow8_qmake_executable}" DIRECTORY)
    endif()
    if(NOT _flow8_qt_bin_hint)
        find_program(_flow8_qmake_tool NAMES qmake6 qmake)
        if(_flow8_qmake_tool)
            execute_process(
                COMMAND "${_flow8_qmake_tool}" -query QT_HOST_BINS
                OUTPUT_VARIABLE _flow8_qt_bin_hint
                OUTPUT_STRIP_TRAILING_WHITESPACE
            )
        endif()
    endif()
    unset(_flow8_lrelease_discovered CACHE)
    find_program(_flow8_lrelease_discovered
        NAMES lrelease6 lrelease-qt6 lrelease
        HINTS "${_flow8_qt_bin_hint}"
        NO_DEFAULT_PATH
    )
    if(NOT _flow8_lrelease_discovered)
        find_program(_flow8_lrelease_discovered
            NAMES lrelease6 lrelease-qt6 lrelease
        )
    endif()
    if(_flow8_lrelease_discovered)
        set(FLOW8_LRELEASE_EXECUTABLE "${_flow8_lrelease_discovered}"
            CACHE FILEPATH "Path to the Qt 6 lrelease executable" FORCE)
    endif()
endif()
if(NOT FLOW8_LRELEASE_EXECUTABLE)
    message(FATAL_ERROR
        "Qt 6 lrelease was not found. Install the Qt Linguist tools "
        "(Ubuntu: qt6-l10n-tools) or set FLOW8_LRELEASE_EXECUTABLE."
    )
endif()
execute_process(
    COMMAND "${FLOW8_LRELEASE_EXECUTABLE}" -version
    RESULT_VARIABLE _flow8_lrelease_result
    OUTPUT_VARIABLE _flow8_lrelease_output
    ERROR_VARIABLE _flow8_lrelease_error
)
if(NOT _flow8_lrelease_result EQUAL 0
   OR NOT "${_flow8_lrelease_output}${_flow8_lrelease_error}" MATCHES "version 6\\.")
    message(FATAL_ERROR
        "FLOW8_LRELEASE_EXECUTABLE must be a working Qt 6 lrelease executable. "
        "Detected: ${_flow8_lrelease_output}${_flow8_lrelease_error}"
    )
endif()

set(FLOW8_TRANSLATIONS_DIR "${CMAKE_CURRENT_BINARY_DIR}/translations")
set(FLOW8_TRANSLATION_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/translations/flow8_en_US.ts"
    "${CMAKE_CURRENT_SOURCE_DIR}/translations/flow8_zh_CN.ts"
)
set(FLOW8_TRANSLATION_BINARIES)
foreach(_flow8_ts IN LISTS FLOW8_TRANSLATION_SOURCES)
    get_filename_component(_flow8_translation_name "${_flow8_ts}" NAME_WE)
    set(_flow8_qm "${FLOW8_TRANSLATIONS_DIR}/${_flow8_translation_name}.qm")
    add_custom_command(
        OUTPUT "${_flow8_qm}"
        COMMAND "${CMAKE_COMMAND}" -E make_directory "${FLOW8_TRANSLATIONS_DIR}"
        COMMAND "${FLOW8_LRELEASE_EXECUTABLE}" "${_flow8_ts}" -qm "${_flow8_qm}"
        DEPENDS "${_flow8_ts}"
        COMMENT "Compiling ${_flow8_translation_name}.qm"
        VERBATIM
    )
    list(APPEND FLOW8_TRANSLATION_BINARIES "${_flow8_qm}")
endforeach()
add_custom_target(flow8_translations ALL DEPENDS ${FLOW8_TRANSLATION_BINARIES})

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
    src/protocol/apk_command_catalog.cpp
    src/protocol/apk_command_catalog.h
    src/protocol/command_codec.cpp
    src/protocol/command_codec.h
    src/protocol/command_stream_decoder.cpp
    src/protocol/command_stream_decoder.h
    src/protocol/codec.cpp
    src/protocol/codec.h
    src/protocol/field_codec.cpp
    src/protocol/field_codec.h
    src/protocol/flow8_command.cpp
    src/protocol/flow8_command.h
    src/protocol/fragment_reassembler.cpp
    src/protocol/fragment_reassembler.h
    src/protocol/flow8_protocol.cpp
    src/protocol/flow8_protocol.h
    src/protocol/gain_codec.cpp
    src/protocol/gain_codec.h
    src/protocol/packet.cpp
    src/protocol/packet.h
    src/protocol/route_level_codec.cpp
    src/protocol/route_level_codec.h
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
    src/core/protocol_state_applier.cpp
    src/core/flow8_transport.cpp
    src/core/flow8_transport.h
    src/core/semantic_command_queue.cpp
    src/core/semantic_command_queue.h
    src/model/flow8_capabilities.cpp
    src/model/flow8_capabilities.h
    src/model/endpoint.h
    src/model/meter.h
    src/model/preferences.h
    src/model/routing.h
    src/model/session.h
    src/model/signal_path.h
    src/model/usb_audio.h
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

add_library(flow8_midi_model STATIC
    src/midi/midi_parameter_map.cpp
    src/midi/midi_parameter_map.h
)
add_library(flow8::midi_model ALIAS flow8_midi_model)
target_include_directories(flow8_midi_model
    PUBLIC
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>"
)
target_link_libraries(flow8_midi_model PUBLIC Qt6::Core)
target_compile_features(flow8_midi_model PUBLIC cxx_std_20)
flow8_enable_warnings(flow8_midi_model)
flow8_configure_platform(flow8_midi_model)

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
    src/ui/channel/channel_edit_view.cpp
    src/ui/channel/channel_edit_view.h
    src/ui/common/flow_layer_bar.cpp
    src/ui/common/flow_layer_bar.h
    src/ui/connection_bar.cpp
    src/ui/connection_bar.h
    src/ui/language_manager.cpp
    src/ui/language_manager.h
    src/ui/layers/layer_views.cpp
    src/ui/layers/layer_views.h
    src/ui/main_window.cpp
    src/ui/main_window.h
    src/ui/ui_text.cpp
    src/ui/ui_text.h
    src/ui/inspector/detail_panel.cpp
    src/ui/inspector/detail_panel.h
    src/ui/inspector/inspector_widget.cpp
    src/ui/inspector/inspector_widget.h
    src/ui/mixer/channel_strip.cpp
    src/ui/mixer/channel_strip.h
    src/ui/mixer/main_strip.cpp
    src/ui/mixer/main_strip.h
    src/ui/mixer/mix_send_strip.cpp
    src/ui/mixer/mix_send_strip.h
    src/ui/mixer/mixer_widget.cpp
    src/ui/mixer/mixer_widget.h
    src/ui/settings/settings_dialog.cpp
    src/ui/settings/settings_dialog.h
    src/ui/session/session_start_view.cpp
    src/ui/session/session_start_view.h
    src/ui/setup/assisted_setup_wizard.cpp
    src/ui/setup/assisted_setup_wizard.h
    src/ui/setup/setup_window.cpp
    src/ui/setup/setup_window.h
    src/ui/stage/stage_view.cpp
    src/ui/stage/stage_view.h
    src/ui/widgets/eq_graph_widget.cpp
    src/ui/widgets/eq_graph_widget.h
    src/ui/widgets/fader_widget.cpp
    src/ui/widgets/fader_widget.h
    src/ui/widgets/knob_widget.cpp
    src/ui/widgets/knob_widget.h
    src/ui/widgets/meter_widget.cpp
    src/ui/widgets/meter_widget.h
)
add_library(flow8::ui ALIAS flow8_ui)
target_include_directories(flow8_ui
    PUBLIC
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>"
)
target_link_libraries(flow8_ui PUBLIC flow8::core Qt6::Widgets)
target_compile_definitions(flow8_ui PRIVATE
    FLOW8_TRANSLATIONS_DIR="${FLOW8_TRANSLATIONS_DIR}"
)
add_dependencies(flow8_ui flow8_translations)
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

    find_package(Threads REQUIRED)
    add_executable(flow8-hardware-bringup tools/hardware_bringup/main.cpp)
    target_link_libraries(flow8-hardware-bringup
        PRIVATE flow8::ble flow8::protocol Qt6::Core Threads::Threads
    )
    flow8_enable_warnings(flow8-hardware-bringup)
    flow8_configure_platform(flow8-hardware-bringup)
endif()

add_executable(flow8-controller src/app/main.cpp)
target_link_libraries(flow8-controller PRIVATE flow8::core flow8::simulator flow8::ui Qt6::Widgets)
target_compile_definitions(flow8-controller PRIVATE
    FLOW8_APP_VERSION="${PROJECT_VERSION}"
)
add_custom_command(TARGET flow8-controller POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E make_directory
        "$<TARGET_FILE_DIR:flow8-controller>/translations"
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
        ${FLOW8_TRANSLATION_BINARIES}
        "$<TARGET_FILE_DIR:flow8-controller>/translations"
    VERBATIM
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
message(STATUS "C++ compiler: ${CMAKE_CXX_COMPILER}")
message(STATUS "C++ compiler ID: ${CMAKE_CXX_COMPILER_ID}")
if(FLOW8_REQUIRE_MINGW)
    message(STATUS "Windows toolchain: Qt MinGW-w64 (required)")
    message(STATUS "Qt mkspec: ${_flow8_qmake_xspec}")
endif()
message(STATUS "Qt version: ${Qt6_VERSION}")
message(STATUS "Qt Bluetooth available: ${FLOW8_HAS_QT_BLUETOOTH}")
