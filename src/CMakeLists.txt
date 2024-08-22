add_executable(${PROJECT_NAME} main.cpp)
target_link_libraries(${PROJECT_NAME} pico_stdlib pico_multicore)

pico_enable_stdio_usb(${PROJECT_NAME} 1)
pico_enable_stdio_uart(${PROJECT_NAME} 0)
pico_add_extra_outputs(${PROJECT_NAME})
