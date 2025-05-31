#!/bin/bash

PROJECT_ROOT="$(cd "$(dirname "$0")"; pwd)"

cmake \
    -DPLATFORM=raspberry-pi \
    -DSUPPORT_CALLBACK_INJECTION=true \
    -B ${PROJECT_ROOT}/can-controller/build \
    ${PROJECT_ROOT}/can-controller

cmake --build ${PROJECT_ROOT}/can-controller/build
