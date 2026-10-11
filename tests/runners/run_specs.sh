#!/usr/bin/env bash
# tests/runners — 用例运行入口（tests/README 约定，工具链维护）。实际逻辑在 ci/run_ci.sh，此脚本为 runner 契约入口。
exec "$(git rev-parse --show-toplevel)/ci/run_ci.sh" "$@"
