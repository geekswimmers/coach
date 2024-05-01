#!/usr/bin/env bash
set -x
set -eo pipefail

DB_USER="${POSTGRES_USER:=geekswimmers_coach_usr}"
DB_PASSWORD="${POSTGRES_PASSWORD:=secret}"
DB_NAME="${POSTGRES_DB:=geekswimmers_coach}"
DB_PORT="${POSTGRES_PORT:=5432}"
DB_HOST="${POSTGRES_HOST:=localhost}"

# Assuming you used the default parameters to launch Postgres in Docker!
DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}
export DATABASE_URL
sqlx migrate add --source storage/migrations $1
