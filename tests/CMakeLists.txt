function(flow8_add_test test_name source_file)
    add_executable(${test_name} ${source_file})
    target_link_libraries(${test_name} PRIVATE
        flow8::core
        flow8::protocol
        flow8::simulator
        Qt6::Test
    )
    flow8_enable_warnings(${test_name})
    flow8_configure_platform(${test_name})
    add_test(NAME ${test_name} COMMAND ${test_name})
endfunction()

flow8_add_test(flow8_state_tests unit/core/flow8_state_test.cpp)
flow8_add_test(flow8_device_tests unit/core/flow8_device_test.cpp)
flow8_add_test(flow8_protocol_tests unit/protocol/protocol_test.cpp)
flow8_add_test(flow8_sysex_tests unit/protocol/sysex_test.cpp)
flow8_add_test(flow8_fake_transport_tests unit/simulator/fake_transport_test.cpp)

if(TARGET flow8::ble)
    add_executable(flow8_ble_services_tests unit/ble/ble_services_test.cpp)
    target_link_libraries(flow8_ble_services_tests PRIVATE flow8::ble Qt6::Test)
    flow8_enable_warnings(flow8_ble_services_tests)
    flow8_configure_platform(flow8_ble_services_tests)
    add_test(NAME flow8_ble_services_tests COMMAND flow8_ble_services_tests)
endif()

add_executable(flow8_gui_smoke_tests unit/ui/gui_smoke_test.cpp)
target_link_libraries(flow8_gui_smoke_tests PRIVATE
    flow8::core
    flow8::simulator
    flow8::ui
    Qt6::Test
    Qt6::Widgets
)
flow8_enable_warnings(flow8_gui_smoke_tests)
flow8_configure_platform(flow8_gui_smoke_tests)
add_test(NAME flow8_gui_smoke_tests COMMAND flow8_gui_smoke_tests)
set_tests_properties(flow8_gui_smoke_tests PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
