#!/usr/bin/env bash
set -euo pipefail

# TODO: Generate publish workflows from Moon Tera templates
# Once .moon/templates/publish-workflows/ is implemented, this script will:
#   moon generate publish-workflows --force
# and place generated files into .github/workflows/
#
# For now, publish workflows are hand-authored under .github/workflows/publish-*.yml
# To add a new SDK publish workflow, copy an existing one and adjust the tag prefix,
# project name, and publish command.

echo "Workflow generation not yet implemented — publish workflows are hand-authored."
echo "See .github/workflows/publish-*.yml"
