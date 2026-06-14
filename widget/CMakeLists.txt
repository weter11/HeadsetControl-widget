find_package(PkgConfig REQUIRED)
find_package(Qt6 REQUIRED COMPONENTS Core DBus Widgets)
pkg_check_modules(UDEV REQUIRED IMPORTED_TARGET libudev)

add_executable(headsetcontrol-widget
    ${CMAKE_CURRENT_SOURCE_DIR}/main.cpp
)

target_include_directories(headsetcontrol-widget PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
)

target_link_libraries(headsetcontrol-widget PRIVATE
    PkgConfig::UDEV
    Qt6::Core
    Qt6::DBus
    Qt6::Widgets
)

install(TARGETS headsetcontrol-widget DESTINATION bin)
install(FILES
    ${CMAKE_CURRENT_SOURCE_DIR}/headsetcontrol-widget.service
    DESTINATION share/headsetcontrol-widget/examples
)
install(FILES
    ${CMAKE_CURRENT_SOURCE_DIR}/../assets/headsetcontrol.png
    DESTINATION share/pixmaps
    RENAME headsetcontrol-widget.png
)
