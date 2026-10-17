#!/bin/bash
# The nightly toolchain with rustfmt, installed only where it is missing. The self-hosted runner image has one (tools/ci/selfhosted/Containerfile),
# in a lower layer of its overlay filesystem: `rustup toolchain install nightly` there UPDATES it as soon as a newer nightly is out, and the update
# renames directories out of that layer ("could not rename 'component' file ... Invalid cross-device link (os error 18)"), failing the job's set-up.
set -eu
if ! rustup toolchain list | grep -q '^nightly-'; then
    rustup toolchain install nightly --profile minimal --component rustfmt
elif ! rustup component list --toolchain nightly --installed | grep -q '^rustfmt-'; then
    rustup component add rustfmt --toolchain nightly
fi
