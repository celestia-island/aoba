# Minimal ARM cross image extension for aoba, used only in CI.
# Base on the official cross image for armv7 / aarch64 as needed.
# Kept under scripts/ to separate CI infra from production source.

# BASE_IMAGE must be provided by the CI build (see workflow).
ARG BASE_IMAGE
FROM ${BASE_IMAGE}

# PC_ARCH must be the Debian multiarch triplet, e.g.:
# - arm-linux-gnueabihf for armv7-unknown-linux-gnueabihf
# - aarch64-linux-gnu    for aarch64-unknown-linux-gnu
ARG PC_ARCH

# Install libudev and pkg-config for libudev-sys builds
RUN apt-get update \
   && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      libudev-dev \
      pkg-config \
   && rm -rf /var/lib/apt/lists/*

# Configure pkg-config for cross-style usage as documented by libudev-sys
# and Autotools Mythbuster: make it sysroot-aware and only search the
# target's pc directories. The sysroot for these images is the container
# root (/), since the container already matches the target ABI.
ENV PKG_CONFIG_DIR="" \
   PKG_CONFIG_ALLOW_CROSS="1" \
   PKG_CONFIG_LIBDIR="/usr/lib/${PC_ARCH}/pkgconfig:/usr/share/pkgconfig" \
   PKG_CONFIG_SYSROOT_DIR="/"
