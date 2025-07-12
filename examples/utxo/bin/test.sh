#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
cd "$PROJECT_ROOT"

echo "Generating typescript client code from OpenAPI spec..."
npm install
#npx @hey-api/openapi-ts -i http://127.0.0.1:8000/apidoc/openapi.json -o client/hey
#cp target/macros/client/client_calls.ts client/index.ts

echo "Executing the client calls against the local server..."
npx tsx client/index.ts