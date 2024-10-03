add_executable(${PROJECT_NAME} main.cpp)
target_link_libraries(${PROJECT_NAME} pico_stdlib pico_multicore cexception)

pico_set_linker_script(${PROJECT_NAME} ${CMAKE_SOURCE_DIR}/scripts/memmap_default.ld)

pico_enable_stdio_usb(${PROJECT_NAME} 1)
pico_enable_stdio_uart(${PROJECT_NAME} 0)

pico_add_extra_outputs(${PROJECT_NAME})
