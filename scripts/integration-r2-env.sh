#!/usr/bin/env bash
# Source for coordinator-run gates only. No credentials, providers or alternate target.
export CARGO_TARGET_DIR=/mnt/linux-extra/moss-cargo-targets/codex-saccade-integ
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=4
export RUSTC_WRAPPER='' CARGO_BUILD_RUSTC_WRAPPER=''
export PKG_CONFIG_PATH=/mnt/linux-extra/saccade-models/toolchain/dav1d/usr/lib/x86_64-linux-gnu/pkgconfig
export SYSTEM_DEPS_DAV1D_BUILD_INTERNAL=never
export PATH=/mnt/linux-extra/saccade-models/toolchain/bin:$PATH
export OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
export CUDA_VISIBLE_DEVICES=''
