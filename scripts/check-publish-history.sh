#!/usr/bin/env sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_root"

case "$(git rev-parse --is-inside-work-tree 2>/dev/null || true)" in
  true) ;;
  *)
    echo "check-publish-history: not inside a git work tree" >&2
    exit 1
    ;;
esac

tmp="${TMPDIR:-/tmp}/solana-tee-history-scan.$$"
trap 'rm -f "$tmp"' EXIT HUP INT TERM

forbidden_path_re='(^|/)(\.keys|target|node_modules|dist)/|(^|/)\.env($|\.)|\.(pem|key|p12|pfx|keystore|jks|asc|gpg|ppk|b58|b64\.key)$|(^|/)id_(rsa|ed25519|ecdsa)(\.|$)|(^|/)(wallet|keypair|id|credentials)\.json$|(^|/).*[-_](wallet|keypair)\.json$|(^|/)keypair.*\.json$|(^|/)(service-account|gcp-credentials|google-credentials|firebase-adminsdk).*\.json$|(^|/).*secret.*\.json$|(^|/)(auth-token|access-token|bearer-token|refresh-token).*\.json$'
allowed_path_re='(^|/)\.env\.(example|template|sample)$'

git rev-list --objects --all |
  sed -n 's/^[^ ][^ ]* //p' |
  grep -E "$forbidden_path_re" |
  grep -Ev "$allowed_path_re" |
  sort -u > "$tmp"

count="$(wc -l < "$tmp" | tr -d ' ')"
if [ "$count" != "0" ]; then
  echo "check-publish-history: found $count forbidden path(s) in git history" >&2
  echo "check-publish-history: first 50 paths:" >&2
  sed -n '1,50p' "$tmp" >&2
  echo "check-publish-history: rewrite or replace history before publishing." >&2
  exit 1
fi

echo "check-publish-history: ok"
