#!/usr/bin/env bash
# Snapshot and reset of a hosted TeamCraft live demo (docs/src/admin/live-demo.md).
#
#   sudo bash demo.sh snapshot   after scripts/seed-demo.sh: dump the demo database
#   sudo bash demo.sh reset      put the database back to that snapshot
#                                (nightly, from cron or a systemd timer)
#
# Runs in the installation directory of deploy/install.sh (INSTALL_DIR,
# default /opt/teamcraft) and talks to the database container – the TeamCraft
# image has no PostgreSQL tools. No Docker volume is removed. The vault stays
# as it is: the demo account cannot write to it, and its cached 3D previews
# stay warm.
#
# Environment: INSTALL_DIR (/opt/teamcraft), SNAPSHOT ($INSTALL_DIR/demo/demo-db.dump).
set -euo pipefail

INSTALL_DIR=${INSTALL_DIR:-/opt/teamcraft}
SNAPSHOT=${SNAPSHOT:-$INSTALL_DIR/demo/demo-db.dump}

cd "$INSTALL_DIR"
if [[ ! -f docker-compose.yml ]]; then
    echo "no docker-compose.yml in $INSTALL_DIR (set INSTALL_DIR)" >&2
    exit 1
fi

case ${1:-} in
snapshot)
    mkdir -p "$(dirname "$SNAPSHOT")"
    docker compose exec -T db pg_dump -U teamcraft -Fc teamcraft >"$SNAPSHOT.tmp"
    mv "$SNAPSHOT.tmp" "$SNAPSHOT"
    echo "snapshot written: $SNAPSHOT ($(wc -c <"$SNAPSHOT") bytes)"
    ;;
reset)
    if [[ ! -s $SNAPSHOT ]]; then
        echo "no snapshot at $SNAPSHOT – run '$0 snapshot' after seeding" >&2
        exit 1
    fi
    echo "$(date -Is) resetting the demo to $SNAPSHOT"
    docker compose stop teamcraft
    # whatever happens below, the demo comes back up
    trap 'docker compose start teamcraft' EXIT
    # An empty schema first, then the snapshot: this also works after an
    # upgrade – the server applies the newer migrations again when it starts,
    # whereas pg_restore --clean would leave their tables behind.
    docker compose exec -T db psql -U teamcraft -d teamcraft -v ON_ERROR_STOP=1 -q \
        -c 'DROP SCHEMA public CASCADE' -c 'CREATE SCHEMA public'
    docker compose exec -T db pg_restore -U teamcraft -d teamcraft --single-transaction --exit-on-error <"$SNAPSHOT"
    echo "$(date -Is) reset done"
    ;;
*)
    echo "usage: $0 snapshot|reset" >&2
    exit 2
    ;;
esac
