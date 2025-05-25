cmake_minimum_required(VERSION 3.10)
project(mission_control)

set(CMAKE_CXX_STANDARD 11)

add_executable(mission_control
    main.cpp
    module_manager.cpp
    queue.cpp
    ${CMAKE_SOURCE_DIR}/../can-controller/api.c
    ${CMAKE_SOURCE_DIR}/../can-controller/mcp2515.c
    )

target_include_directories(
    mission_control PRIVATE ${CMAKE_SOURCE_DIR}/..)

find_library(WIRINGPI_LIB wiringPi REQUIRED)

target_link_libraries(mission_control ${WIRINGPI_LIB})