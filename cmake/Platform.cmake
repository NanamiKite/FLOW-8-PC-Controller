function(flow8_configure_platform target_name)
    if(WIN32)
        target_compile_definitions(${target_name} PRIVATE
            NOMINMAX
            UNICODE
            _UNICODE
        )
    endif()
endfunction()
