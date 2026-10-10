#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

set -euo pipefail
script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$script_directory/../../.."
repository_root=$PWD
export PARITY_REQUIRE_REFS=1
export RUST_MIN_STACK=${RUST_MIN_STACK:-16777216}
export PARITY_ARTIFACT_DIR=${PARITY_ARTIFACT_DIR:-"$repository_root/target/parity-artifacts"}

run_binary() {
    local binary=$1
    local names=()
    mapfile -t names < <(xvfb-run -a "$binary" --list | sed -n 's/: test$//p')
    if ((${#names[@]} == 0)); then
        echo 'No parity tests found' >&2
        return 1
    fi
    local failed=0
    cd "$repository_root/tests"
    for ((start=0; start<${#names[@]}; start+=40)); do
        echo "Parity tests $((start + 1))-$((start + 40 < ${#names[@]} ? start + 40 : ${#names[@]})) of ${#names[@]}"
        if ! xvfb-run -a "$binary" --test-threads=4 --exact "${names[@]:start:40}"; then
            failed=1
        fi
    done
    cd "$repository_root"
    return "$failed"
}

if (($# == 1)); then
    binary=$(realpath -- "$1")
    run_binary "$binary"
    exit
elif (($# != 0)); then
    echo 'Usage: check-parity.sh [existing-test-binary]' >&2
    exit 2
fi

mapfile -t cases < <(find tests/screenshots/cases/material -name '*.slint' | LC_ALL=C sort)
if ((${#cases[@]} == 0)); then
    echo 'No Material parity cases found' >&2
    exit 1
fi
build_messages=$(mktemp)
trap 'rm -f -- "$build_messages"' EXIT
failed=0
for ((shard=0; shard<6; shard++)); do
    selected=()
    for ((index=shard; index<${#cases[@]}; index+=6)); do
        selected+=("${cases[index]#tests/screenshots/cases/}")
    done
    ((${#selected[@]})) || continue
    filter=$(IFS=,; echo "${selected[*]}")
    echo "Compiling parity shard $((shard + 1))/6 (${#selected[@]} scenes)"
    SLINT_TEST_FILTER="$filter" cargo --config profile.dev.package.i-slint-compiler.opt-level=3 \
        --config profile.dev.package.test-driver-screenshots.debug=0 \
        test --manifest-path tests/Cargo.toml -p test-driver-screenshots \
        --features femtovg --no-run --message-format=json-render-diagnostics > "$build_messages"
    binary=$(python3 - "$build_messages" <<'PY'
import json
import sys
binary = None
with open(sys.argv[1]) as messages:
    for line in messages:
        message = json.loads(line)
        if (message.get('reason') == 'compiler-artifact'
                and message.get('profile', {}).get('test')
                and message.get('target', {}).get('name') == 'test-driver-screenshot'
                and message.get('executable')):
            binary = message['executable']
if binary is None:
    sys.exit('No parity test executable produced')
print(binary)
PY
    )
    if ! run_binary "$binary"; then
        failed=1
    fi
done
exit "$failed"
