#!/bin/sh
set -eu

tambo_bin=${1:-tambo}

"$tambo_bin" --version >/dev/null
for command in quickstart init plan apply verify install project-settings verify-questions render-observability validate-observability queue-br flush-episodes; do
    "$tambo_bin" "$command" --help >/dev/null
done

"$tambo_bin" init --help | grep -q -- '--guided'
"$tambo_bin" plan --help | grep -q -- '--profile'
"$tambo_bin" plan --help | grep -q -- '--intent'
"$tambo_bin" apply --help | grep -q -- '--skip-install'
"$tambo_bin" verify --help | grep -q -- '--state'
"$tambo_bin" verify --help | grep -q -- '--creel-doctor'
"$tambo_bin" verify --help | grep -q -- '--creel-admission'
"$tambo_bin" install --help | grep -q -- '--creel-doctor'
"$tambo_bin" install --help | grep -q -- '--creel-admission'
"$tambo_bin" verify-questions --help | grep -q -- '--plan'

printf '%s\n' 'tambo skill surface: verified'
