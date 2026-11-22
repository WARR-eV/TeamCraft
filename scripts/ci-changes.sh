#!/usr/bin/env bash
# Which CI jobs a change needs: the one table from changed files to the jobs
# of ci.yml, desktop.yml, codeql.yml and docs.yml (CONTRIBUTING.md, "What CI
# runs for a change"). A job runs only when a file it compiles, tests or reads
# changed.
#
# In a workflow, after actions/checkout with fetch-depth: 2:
#   scripts/ci-changes.sh
# finds the files the event changed and writes `name=value` lines to
# $GITHUB_OUTPUT (and stdout). The base of a push or a merge-queue entry comes
# from CI_PUSH_BEFORE / CI_MERGE_BASE (github.event.before,
# github.event.merge_group.base_sha).
#
# Locally:
#   git diff --name-only origin/main... | scripts/ci-changes.sh --files
#   scripts/ci-changes.sh --self-test
#
# Everything runs for a scheduled or manual run, when the changed files cannot
# be found (first push of a branch, force push) and when this script changes.
# The weekly full runs of ci.yml, desktop.yml and codeql.yml catch a file this
# table misses. When a test starts to read a new file outside its area, add the
# file here.
set -euo pipefail

# ---- the table ---------------------------------------------------------------

# server – ci.yml: fmt, clippy and rustdoc; the test suite on PostgreSQL 16/18;
# the Windows and macOS builds. Everything the server compiles in or its tests
# read: sources, tests and fixtures, migrations, templates, the web and shared
# assets, logos and icons (src/assets.rs), the vendored crates, the fuzz seeds
# (tests/parser_corpus.rs), the build configuration – and the files of the
# desktop app that server tests compare with their own (src/assets.rs,
# src/domain/mod.rs, tests/branding_names.rs, tests/desktop_compat.rs,
# src/services/usage.rs).
SERVER='^(src|tests|migrations|templates|static|shared|vendor|media/logos|media/icons|fuzz/seeds)/'
SERVER+='|^(Cargo\.toml|Cargo\.lock|build\.rs|clippy\.toml|rustfmt\.toml|\.rustfmt\.toml|rust-toolchain|rust-toolchain\.toml)$'
SERVER+='|^\.cargo/config(\.toml)?$|^\.config/nextest\.toml$|^\.github/coverage-floor$|^\.github/workflows/ci\.yml$'
SERVER+='|^desktop/ui/(app\.css|index\.html)$|^desktop/ui/js/(core|settings|shell)\.js$|^desktop/src/(compat|usage)\.rs$'

# guards – ci.yml, without a database: config::tests and
# tests/architecture_guards.rs, for the files outside the server area they
# read: every page of the book (each configuration variable is documented),
# the shipped configuration examples and the desktop Group Policy template
# (its language files must match the ADMX). Runs only when `server` does not.
GUARDS='^docs/src/|^deploy/(teamcraft\.env\.example|docker-compose\.yml)$|^deploy/desktop-policies/windows/|^packaging/linux/teamcraft\.env$'

# cargo deny – ci.yml, one leg per lock file.
DENY_SERVER='^(Cargo\.toml|Cargo\.lock|deny\.toml)$|^vendor/|^\.github/workflows/ci\.yml$'
DENY_DESKTOP='^desktop/Cargo\.(toml|lock)$|^deny\.toml$|^\.github/workflows/ci\.yml$'

# desktop_rust – desktop.yml: fmt, clippy, tests, build and start-up smoke
# test on every platform. The app's Rust code, configuration and icons, the
# shared/ mirror and the media/ logos (desktop/build.rs copies both), the NIST
# files of the folder-import test, the lint configuration of the repository
# root – but not the scripts and styles in desktop/ui/ (the ui job checks
# them), except desktop/ui/index.html, which a Rust test reads.
DESKTOP_RUST='^(desktop|shared|media/logos|media/icons|tests/fixtures/cad/nist)/'
DESKTOP_RUST+='|^(clippy\.toml|rustfmt\.toml|\.rustfmt\.toml|rust-toolchain|rust-toolchain\.toml)$|^\.github/workflows/desktop\.yml$'
DESKTOP_RUST_NOT='^desktop/ui/|\.md$'
DESKTOP_RUST_ALSO='^desktop/ui/index\.html$'

# desktop_ui – desktop.yml: syntax of every script of the desktop and web UI,
# the imports of the desktop modules and the Node tests in scripts/.
DESKTOP_UI='^(desktop/ui|static|shared)/|^scripts/[^/]*\.mjs$|^\.github/workflows/desktop\.yml$'
# scripts/desktop-lock-layout.test.mjs compares the desktop layout with the
# server's sizes in this file (#428)
DESKTOP_UI+='|^src/services/lock_overview\.rs$'

# codeql – codeql.yml, one analysis per language.
CODEQL_ACTIONS='^\.github/(workflows|actions|codeql)/'
CODEQL_JS='\.(js|mjs|cjs|ts|html|htm)$|^\.github/codeql/|^\.github/workflows/codeql\.yml$'
CODEQL_RUST='\.rs$|(^|/)Cargo\.(toml|lock)$|^vendor/|^\.github/codeql/|^\.github/workflows/codeql\.yml$'

# docs_site – docs.yml: the book and the rustdoc API reference (the links
# check runs whenever docs.yml runs at all).
DOCS_SITE='^docs/|^src/|^Cargo\.(toml|lock)$|^\.github/workflows/docs\.yml$'

# ---- matching ----------------------------------------------------------------

FILES=''
ALL=false

# has <include> [<exclude>]: a changed file matches <include> and not
# <exclude> (extended regular expressions); always true with ALL.
has() {
    if [ "$ALL" = true ]; then return 0; fi
    local hit
    hit=$(grep -E -- "$1" <<<"$FILES" || true)
    if [ -n "$hit" ] && [ -n "${2:-}" ]; then
        hit=$(grep -vE -- "$2" <<<"$hit" || true)
    fi
    [ -n "$hit" ]
}

flag() {
    if has "$@"; then echo true; else echo false; fi
}

# json_list <name> <include> [<name> <include> …]: a JSON array of the names
# whose files changed, for a matrix (fromJSON).
json_list() {
    local out=''
    while [ "$#" -ge 2 ]; do
        if has "$2"; then out+="${out:+,}\"$1\""; fi
        shift 2
    done
    echo "[$out]"
}

# classify <files, one per line> <all: true|false>: the name=value lines.
classify() {
    FILES=$(grep -v '^$' <<<"$1" || true)
    ALL=$2
    if [ "$ALL" != true ] && grep -qE '^scripts/ci-changes\.sh$' <<<"$FILES"; then
        ALL=true
    fi
    local desktop_rust=false
    if has "$DESKTOP_RUST" "$DESKTOP_RUST_NOT" || has "$DESKTOP_RUST_ALSO"; then
        desktop_rust=true
    fi
    echo "all=$ALL"
    echo "server=$(flag "$SERVER")"
    echo "guards=$(flag "$GUARDS")"
    echo "deny=$(json_list Cargo.toml "$DENY_SERVER" desktop/Cargo.toml "$DENY_DESKTOP")"
    echo "desktop_rust=$desktop_rust"
    echo "desktop_ui=$(flag "$DESKTOP_UI")"
    echo "codeql=$(json_list actions "$CODEQL_ACTIONS" javascript-typescript "$CODEQL_JS" rust "$CODEQL_RUST")"
    echo "docs_site=$(flag "$DOCS_SITE")"
}

# ---- the files of the event --------------------------------------------------

# The changed files, one per line; fails when they cannot be found (then
# everything runs).
changed_files() {
    local base
    case "${GITHUB_EVENT_NAME:-}" in
        # the merge commit of the pull request: its first parent is the base
        pull_request) git -c core.quotepath=off diff --name-only HEAD^1 HEAD; return ;;
        push) base=${CI_PUSH_BEFORE:-} ;;
        merge_group) base=${CI_MERGE_BASE:-} ;;
        # schedule, workflow_dispatch: everything
        *) return 1 ;;
    esac
    # a new branch or a force push has no usable base
    case "$base" in '' | *[!0-9a-f]* | 0000000000000000000000000000000000000000) return 1 ;; esac
    git fetch --quiet --no-tags --depth=1 origin "$base" || return 1
    git -c core.quotepath=off diff --name-only "$base" HEAD
}

# ---- self-test ---------------------------------------------------------------

FAILS=0

# check <files, comma-separated> <name=value>…
check() {
    local files out kv
    files=$(tr ',' '\n' <<<"$1")
    shift
    out=$(classify "$files" false)
    for kv in "$@"; do
        if ! grep -qxF -- "$kv" <<<"$out"; then
            echo "FAIL: [$files] expected $kv, got:" >&2
            sed 's/^/  /' <<<"$out" >&2
            FAILS=$((FAILS + 1))
        fi
    done
}

self_test() {
    # desktop UI scripts: the UI job and the JavaScript analysis only
    check desktop/ui/js/graph.js server=false desktop_rust=false desktop_ui=true guards=false \
        'codeql=["javascript-typescript"]' 'deny=[]' docs_site=false
    # … except the files server tests compare with their own
    check desktop/ui/js/core.js server=true desktop_rust=false desktop_ui=true
    check desktop/ui/app.css server=true desktop_rust=false desktop_ui=true 'codeql=[]'
    check desktop/ui/index.html server=true desktop_rust=true desktop_ui=true
    # desktop Rust code
    check desktop/src/workspace.rs server=false desktop_rust=true desktop_ui=false 'codeql=["rust"]' 'deny=[]'
    check desktop/src/compat.rs server=true desktop_rust=true
    check desktop/src/usage.rs server=true desktop_rust=true
    check desktop/Cargo.lock server=false desktop_rust=true 'deny=["desktop/Cargo.toml"]' 'codeql=["rust"]'
    check desktop/tauri.conf.json server=false desktop_rust=true desktop_ui=false 'codeql=[]'
    check desktop/README.md server=false desktop_rust=false desktop_ui=false
    # server
    check src/lib.rs server=true desktop_rust=false desktop_ui=false guards=false 'codeql=["rust"]' 'deny=[]' docs_site=true
    check tests/web_ui.rs server=true docs_site=false 'codeql=["rust"]'
    check migrations/0099_example.sql server=true 'codeql=[]' docs_site=false
    check templates/base.html server=true desktop_ui=false 'codeql=["javascript-typescript"]'
    check Cargo.lock server=true desktop_rust=false 'deny=["Cargo.toml"]' 'codeql=["rust"]'
    check vendor/openskp/src/lib.rs server=true 'deny=["Cargo.toml"]' 'codeql=["rust"]'
    check fuzz/seeds/invpart/zstd-segments.ipt server=true 'codeql=[]'
    check deny.toml server=false desktop_rust=false 'deny=["Cargo.toml","desktop/Cargo.toml"]'
    # shared by the server and the desktop app
    check shared/js/viewer.js server=true desktop_rust=true desktop_ui=true 'codeql=["javascript-typescript"]'
    check shared/README.md server=true desktop_rust=false desktop_ui=true
    check media/logos/TeamCraft_Icon.svg server=true desktop_rust=true desktop_ui=false
    check tests/fixtures/cad/nist/ctc-01/inventor/ctc_01.ipt server=true desktop_rust=true
    check clippy.toml server=true desktop_rust=true
    check scripts/desktop-tab-order.test.mjs server=false desktop_ui=true 'codeql=["javascript-typescript"]'
    check src/services/lock_overview.rs server=true desktop_ui=true desktop_rust=false
    # documentation and examples read by the guard tests
    check docs/src/admin/configuration.md server=false guards=true docs_site=true 'codeql=[]'
    check deploy/teamcraft.env.example server=false guards=true
    check packaging/linux/teamcraft.env server=false guards=true
    check deploy/desktop-policies/windows/de-DE/TeamCraft.adml server=false guards=true 'codeql=[]'
    # nothing in these workflows
    check README.md all=false server=false guards=false desktop_rust=false desktop_ui=false \
        'codeql=[]' 'deny=[]' docs_site=false
    check media/screenshots/vault.png server=false desktop_rust=false desktop_ui=false 'codeql=[]'
    check deploy/install.sh server=false guards=false 'codeql=[]'
    check '' all=false server=false 'codeql=[]' 'deny=[]'
    # workflows
    check .github/workflows/release.yml server=false desktop_rust=false 'codeql=["actions"]' 'deny=[]'
    check .github/workflows/ci.yml server=true desktop_rust=false 'deny=["Cargo.toml","desktop/Cargo.toml"]' 'codeql=["actions"]'
    check .github/workflows/desktop.yml server=false desktop_rust=true desktop_ui=true
    check .github/workflows/codeql.yml server=false 'codeql=["actions","javascript-typescript","rust"]'
    check .github/workflows/docs.yml docs_site=true server=false
    # several areas at once
    check src/lib.rs,desktop/ui/js/graph.js server=true desktop_ui=true desktop_rust=false \
        'codeql=["javascript-typescript","rust"]'
    # this table: everything
    check scripts/ci-changes.sh all=true server=true guards=true desktop_rust=true desktop_ui=true \
        'deny=["Cargo.toml","desktop/Cargo.toml"]' 'codeql=["actions","javascript-typescript","rust"]' docs_site=true
    if [ "$FAILS" -gt 0 ]; then
        echo "ci-changes self-test: $FAILS failed" >&2
        return 1
    fi
    echo "ci-changes self-test: passed"
}

# ---- main --------------------------------------------------------------------

case "${1:-}" in
    --self-test)
        self_test
        exit
        ;;
    --files)
        classify "$(cat)" false
        exit
        ;;
    --all) out=$(classify '' true) ;;
    '')
        if files=$(changed_files); then
            echo "changed files:"
            sed 's/^/  /' <<<"$files"
            out=$(classify "$files" false)
        else
            echo "changed files unknown (event ${GITHUB_EVENT_NAME:-none}): every job runs"
            out=$(classify '' true)
        fi
        ;;
    *)
        echo "usage: $0 [--files | --all | --self-test]" >&2
        exit 2
        ;;
esac
echo "$out"
if [ -n "${GITHUB_OUTPUT:-}" ]; then
    echo "$out" >>"$GITHUB_OUTPUT"
fi
