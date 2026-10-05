#!/bin/sh
# Always load release settings before host-generated networking settings.
set -eu
cd "$(dirname "$0")/../.."
exec docker compose --env-file .env --env-file deploy/generated/compose.env \
  -f docker-compose.yml "$@"
