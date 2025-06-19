#!/bin/bash

PROJECT_ROOT="$(cd "$(dirname "$0")"; pwd)"
rm -r ${PROJECT_ROOT}/can-controller/build
git submodule update --recursive
mkdir -p ${PROJECT_ROOT}/can-controller/build

cmake \
    -DPLATFORM=raspberry-pi \
    -DSUPPORT_CALLBACK_INJECTION=true \
    -B ${PROJECT_ROOT}/can-controller/build \
    ${PROJECT_ROOT}/can-controller

cmake --build ${PROJECT_ROOT}/can-controller/build
