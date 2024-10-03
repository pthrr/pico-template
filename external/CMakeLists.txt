include(ExternalProject)

set(MESON_PROJECT_SOURCE_DIR ${CMAKE_CURRENT_SOURCE_DIR}/CException)
set(MESON_PROJECT_BUILD_DIR ${CMAKE_BINARY_DIR}/CException)

ExternalProject_Add(
    CException
    SOURCE_DIR ${MESON_PROJECT_SOURCE_DIR}
    CONFIGURE_COMMAND meson setup ${MESON_PROJECT_BUILD_DIR} ${MESON_PROJECT_SOURCE_DIR}
    BUILD_COMMAND meson compile -C ${MESON_PROJECT_BUILD_DIR}
    INSTALL_COMMAND ""
    BUILD_BYPRODUCTS ${MESON_PROJECT_BUILD_DIR}/CException/lib/libcexception.a
)

add_library(cexception_lib STATIC IMPORTED)
set_target_properties(cexception_lib PROPERTIES
    IMPORTED_LOCATION ${MESON_PROJECT_BUILD_DIR}/lib/libcexception.a
)
target_include_directories(cexception_lib INTERFACE ${MESON_PROJECT_SOURCE_DIR}/lib)

add_library(cexception INTERFACE)
target_link_libraries(cexception INTERFACE cexception_lib)
