#!/bin/bash

PROJECT_ROOT="$(cd "$(dirname "$0")"; pwd)"
rm -r ${PROJECT_ROOT}/can-controller/build
git submodule update --recursive
mkdir -p ${PROJECT_ROOT}/can-controller/build

cmake \
    -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DPLATFORM=raspberry-pi \
    -DDEVICE=mcp2518fd \
    -DSUPPORT_CALLBACK_INJECTION=true \
    -B ${PROJECT_ROOT}/can-controller/build \
    ${PROJECT_ROOT}/can-controller

cmake --build ${PROJECT_ROOT}/can-controller/build
