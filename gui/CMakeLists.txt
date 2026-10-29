cmake_minimum_required(VERSION 3.21)
project(yinyang-gui VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTOUIC ON)

# Public Qt API only: the GUI follows the KDE theme through plasma-integration, with no KDE Frameworks.
find_package(Qt6 6.5 REQUIRED COMPONENTS Widgets Test LinguistTools)

add_library(yinyang_gui STATIC
    coreclient.cpp
    coreclient.h
    mainwindow.cpp
    mainwindow.h
    main_window.ui
    appsetup.cpp
    appsetup.h
    tray.cpp
    tray.h
)
qt_add_resources(yinyang_gui icons
    PREFIX /icons
    FILES icon.svg
)
# The .ts files come from Yin-Yang 4.0.1; entries still marked unfinished stay English.
qt_add_translations(yinyang_gui
    TS_FILES
        translations/yin_yang.de_DE.ts
        translations/yin_yang.nl_NL.ts
        translations/yin_yang.zh_CN.ts
        translations/yin_yang.zh_TW.ts
    RESOURCE_PREFIX /i18n
)
target_link_libraries(yinyang_gui PUBLIC Qt6::Widgets)
target_include_directories(yinyang_gui PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})

add_executable(yinyang-gui main.cpp)
target_compile_definitions(yinyang-gui PRIVATE PROJECT_VERSION="${PROJECT_VERSION}")
target_link_libraries(yinyang-gui PRIVATE yinyang_gui)

enable_testing()
foreach(name coreclient mainwindow tray)
    add_executable(tst_${name} tests/tst_${name}.cpp)
    target_link_libraries(tst_${name} PRIVATE yinyang_gui Qt6::Test)
    target_compile_definitions(tst_${name} PRIVATE
        FAKE_YINYANG="${CMAKE_CURRENT_SOURCE_DIR}/tests/fake_yinyang.sh")
    add_test(NAME ${name} COMMAND tst_${name})
    set_tests_properties(${name} PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
endforeach()
