cmake_minimum_required(VERSION 3.20)
project(trusttunnel_bridge)

add_library(trusttunnel_bridge SHARED
    src/trusttunnel_bridge.cpp
)

target_include_directories(trusttunnel_bridge PUBLIC include)
