set(CMAKE_VERBOSE_MAKEFILE OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

set(CMAKE_CXX_SCAN_FOR_MODULES OFF)

find_program(CCACHE_PROGRAM sccache)

if(CCACHE_PROGRAM)
  set_property(GLOBAL PROPERTY RULE_LAUNCH_COMPILE "${CCACHE_PROGRAM}")
  message(STATUS "Using sccache")
endif()

set(PICO_PLATFORM rp2040)
set(PICO_COMPILER pico_arm_gcc)
set(PICO_BOARD pico)

set(CMAKE_CXX_STANDARD_REQUIRED True)
set(CMAKE_CXX_STANDARD 23)

message(STATUS "Compiler ID is: ${CMAKE_CXX_COMPILER_ID}")
message(STATUS "Build type is: ${CMAKE_BUILD_TYPE}")

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
  add_compile_options(-ftrivial-auto-var-init=pattern)
  add_compile_options(-O0 -g)
endif()
