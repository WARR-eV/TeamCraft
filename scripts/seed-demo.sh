#!/usr/bin/env bash
# Loads the demo data of the README screenshots into a fresh TeamCraft and
# adds the public, read-only account of a hosted live demo
# (docs/src/admin/live-demo.md): two editors, the project group ROVER with the
# NIST CAD models, a multi-level BOM, a released and a pending change order, a
# change request, two simulations with results, the NX model of the
# enclosure, an as-built unit and the restricted group LAB. Builds nothing.
#
#   BASE_URL=http://127.0.0.1:8080 ADMIN_PASSWORD_FILE=/opt/teamcraft/secrets/admin_password \
#     DEMO_PASSWORD=try-teamcraft scripts/seed-demo.sh
#
# Environment:
#   BASE_URL             TeamCraft address                    (http://127.0.0.1:8080)
#   ADMIN_PASSWORD_FILE  password of the existing administrator "admin"
#                        (TEAMCRAFT_BOOTSTRAP_ADMIN_*); unset: the first-start
#                        page creates it, which only a server on localhost offers,
#                        with the one-time code of its setup link in SETUP_CODE
#   DEMO_USER            the public account               (demo; = TEAMCRAFT_DEMO_USER)
#   DEMO_PASSWORD        its password, at least 10 characters (= TEAMCRAFT_DEMO_PASSWORD)
#   FIXTURES             NIST CAD files                       (tests/fixtures/cad/nist)
#
# Run it once against an EMPTY database, from the repository root (curl and
# jq needed), then take the snapshot the nightly reset goes back to
# (deploy/demo/demo.sh snapshot). The other passwords and tokens are generated
# per run and never printed.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
DEMO_USER=${DEMO_USER:-demo}
DEMO_PASSWORD=${DEMO_PASSWORD:?set DEMO_PASSWORD to the value of TEAMCRAFT_DEMO_PASSWORD}
if [[ ${#DEMO_PASSWORD} -lt 10 ]]; then
    echo "DEMO_PASSWORD must have at least 10 characters" >&2
    exit 2
fi

# The seeding of the screenshot script; sourced, it captures nothing.
# shellcheck source=scripts/screenshots.sh
source "$here/screenshots.sh"

# demo_account – the public account: a CAD editor of ROVER (so the pages show
# what an editor sees; the server refuses its changes), not a member of LAB
# (so the demo shows how a restricted group stays hidden).
demo_account() {
    local code
    log "creating the public demo account $DEMO_USER"
    printf '%s' "$DEMO_PASSWORD" >"$WORK/pw-$DEMO_USER"
    code=$(web admin /admin/users --data-urlencode "username=$DEMO_USER" \
        --data-urlencode "display_name=Demo visitor" \
        --data-urlencode role=editor \
        --data-urlencode "password@$WORK/pw-$DEMO_USER")
    [[ $code == 303 || $code == 302 ]] || die "creating $DEMO_USER failed (HTTP $code)"
    japi admin POST /projects/ROVER/members "$(jq -nc --arg u "$DEMO_USER" '{username: $u}')" >/dev/null
    # the account signs in with the password the login page shows
    login_web "$DEMO_USER"
}

wait_for_server
seed
seed_change_request
seed_simulation
seed_nx
seed_as_built
seed_restricted
demo_account
log "demo data ready at $BASE_URL – sign in as $DEMO_USER; next: deploy/demo/demo.sh snapshot"
