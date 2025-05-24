cmake_minimum_required(VERSION 3.10)
project(mcp2515_init)

set(CMAKE_CXX_STANDARD 11)

add_executable(mission_control
    main.cpp
    mcp2515_impl.cpp
    module_manager.cpp
    queue.cpp)

find_library(WIRINGPI_LIB wiringPi REQUIRED)

target_link_libraries(mission_control ${WIRINGPI_LIB})