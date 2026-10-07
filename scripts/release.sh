#!/usr/bin/env bash
# Publish the next release of Snip on GitHub (D33): tag the given commit
# (default HEAD) with the next version and let GitHub write the notes from
# the pull requests merged since the last release.
#
#   scripts/release.sh [patch|minor|major] [commit]
#
# DRY_RUN=1 only prints what it would do. The release workflow runs this on
# every push to main; it also works by hand (gh must be logged in).
set -euo pipefail

bump=${1:-patch}
commit=${2:-$(git rev-parse HEAD)}

last=$(git tag --list 'v*' --sort=-v:refname | head -n1)
if [[ -z $last ]]; then
  # the first release takes the workspace's version as it is
  next=v$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
else
  IFS=. read -r major minor patch <<<"${last#v}"
  case $bump in
  major) next=v$((major + 1)).0.0 ;;
  minor) next=v$major.$((minor + 1)).0 ;;
  patch) next=v$major.$minor.$((patch + 1)) ;;
  *) echo "release.sh: bump must be patch, minor or major" >&2 && exit 2 ;;
  esac
fi

if git rev-parse -q --verify "refs/tags/$next" >/dev/null; then
  echo "release.sh: $next exists already" >&2
  exit 1
fi

echo "Snip $next at ${commit:0:12} (last release: ${last:-none})"
if [[ ${DRY_RUN:-} == 1 ]]; then
  exit 0
fi
gh release create "$next" --target "$commit" --title "Snip $next" --generate-notes ${last:+--notes-start-tag "$last"}
