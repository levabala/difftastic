#!/bin/bash
exec /Users/lev.balagurov/projects/difftastic/target/release/difft \
  --background-diff-colors on \
  --diff-color-added-bg "#003500" \
  --diff-color-removed-bg "#440000" \
  "$@"
