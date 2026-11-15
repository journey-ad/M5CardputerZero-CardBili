#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

shellcheck scripts/*.sh .agents/skills/cardputer-zero/scripts/*.sh
python3 -m py_compile scripts/*.py .agents/skills/cardputer-zero/scripts/fbcap.py
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.12 -color
python3 scripts/check-release.py
docker run --rm -v "$PWD:/repo:ro" -w /repo \
    -e GIT_CONFIG_COUNT=1 \
    -e GIT_CONFIG_KEY_0=safe.directory \
    -e GIT_CONFIG_VALUE_0=/repo \
    --entrypoint sh zricethezav/gitleaks:v8.30.1 \
    -c 'git rev-parse --verify HEAD >/dev/null && gitleaks git --redact --no-banner --log-opts=--all .'
