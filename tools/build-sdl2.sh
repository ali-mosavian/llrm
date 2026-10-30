#!/usr/bin/env bash
# Builds SDL2 into ~/.cache/llrm/sdl2 (SDL2_PREFIX overrides) for the QB host runtime's sdl platform when the machine
# has no SDL2 development package. The build has no X11, Wayland or sound-server backend, so it opens the dummy video
# and audio drivers only (SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy): enough for tests/qb_sdl.rs, not for a window.
# On a desktop install SDL2 with the system's package manager (apt install libsdl2-dev, brew install sdl2) instead.
set -euo pipefail

version=2.30.9
sha256=24b574f71c87a763f50704bbb630cbe38298d544a1f890f099a4696b1d6beba4
prefix=${SDL2_PREFIX:-$HOME/.cache/llrm/sdl2}
if [ -x "$prefix/bin/sdl2-config" ]; then
    echo "$prefix/bin/sdl2-config"
    exit 0
fi
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -sSfL -o "$work/sdl2.tar.gz" "https://github.com/libsdl-org/SDL/releases/download/release-$version/SDL2-$version.tar.gz"
echo "$sha256  $work/sdl2.tar.gz" | sha256sum -c - >&2
mkdir "$work/src"
tar xzf "$work/sdl2.tar.gz" -C "$work/src" --strip-components=1
cmake -S "$work/src" -B "$work/build" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" \
    -DSDL_SHARED=ON -DSDL_STATIC=OFF -DSDL_TEST=OFF -DSDL_X11=OFF -DSDL_WAYLAND=OFF -DSDL_ALSA=OFF \
    -DSDL_PULSEAUDIO=OFF -DSDL_PIPEWIRE=OFF -DSDL_JACK=OFF -DSDL_OPENGL=OFF -DSDL_OPENGLES=OFF -DSDL_VULKAN=OFF \
    -DSDL_KMSDRM=OFF -DSDL_RPATH=OFF >&2
cmake --build "$work/build" -j"$(nproc)" >&2
cmake --install "$work/build" >&2
echo "$prefix/bin/sdl2-config"
