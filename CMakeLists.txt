cmake_minimum_required(VERSION 3.27 FATAL_ERROR)
list(APPEND CMAKE_MODULE_PATH ${CMAKE_CURRENT_SOURCE_DIR}/cmake)

include(${CMAKE_CURRENT_SOURCE_DIR}/cmake/ProjectVersion.cmake)
include(external/pico-sdk/pico_sdk_init.cmake)

project(
  my_project
  LANGUAGES ASM CXX C
  VERSION ${PROJECT_VERSION}
  DESCRIPTION "My project")

pico_sdk_init()

include(${CMAKE_CURRENT_SOURCE_DIR}/cmake/ProjectSettings.cmake)

file(GLOB_RECURSE ${PROJECT_NAME}_SOURCES CONFIGURE_DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/src/*.cpp)
file(GLOB_RECURSE ${PROJECT_NAME}_HEADER CONFIGURE_DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/src/*.h)

add_subdirectory(src)

execute_process(
    COMMAND ${CMAKE_COMMAND} -E create_symlink
    ${CMAKE_BINARY_DIR}/compile_commands.json
    ${CMAKE_SOURCE_DIR}/build/compile_commands.json
)
