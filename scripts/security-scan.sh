#!/usr/bin/env sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_root"

case "$(git rev-parse --is-inside-work-tree 2>/dev/null || true)" in
  true) ;;
  *)
    echo "security-scan: not inside a git work tree" >&2
    exit 1
    ;;
esac

forbidden_path_re='(^|/)(\.keys|target|node_modules|dist)/|(^|/)\.env($|\.)|\.(pem|key|p12|pfx|keystore|jks|asc|gpg|ppk|b58|b64\.key)$|(^|/)id_(rsa|ed25519|ecdsa)(\.|$)|(^|/)(wallet|keypair|id|credentials)\.json$|(^|/).*[-_](wallet|keypair)\.json$|(^|/)keypair.*\.json$|(^|/)(service-account|gcp-credentials|google-credentials|firebase-adminsdk).*\.json$|(^|/).*secret.*\.json$|(^|/)(auth-token|access-token|bearer-token|refresh-token).*\.json$'
allowed_path_re='(^|/)\.env\.(example|template|sample)$'

echo "security-scan: checking tracked filenames"
tracked_matches="$(
  git ls-files | grep -E "$forbidden_path_re" | grep -Ev "$allowed_path_re" || true
)"

if [ -n "$tracked_matches" ]; then
  echo "security-scan: forbidden tracked paths:" >&2
  printf '%s\n' "$tracked_matches" >&2
  exit 1
fi

if ! command -v rg >/dev/null 2>&1; then
  echo "security-scan: ripgrep (rg) is required" >&2
  exit 1
fi

echo "security-scan: scanning non-secret source files for secret-like patterns"
matches="$(
  rg -l --hidden \
    -g '!.git/**' \
    -g '!.keys/**' \
    -g '!target/**' \
    -g '!**/target/**' \
    -g '!node_modules/**' \
    -g '!**/node_modules/**' \
    -g '!dist/**' \
    -g '!**/dist/**' \
    -e 'sk-[A-Za-z0-9_-]{20,}' \
    -e 'nvapi-[A-Za-z0-9_-]+' \
    -e '-----BEGIN (RSA |EC |OPENSSH |PRIVATE )?PRIVATE KEY-----' \
    -e '\[[0-9]{1,3}(,[0-9]{1,3}){31,}\]' \
    -e 'SOLANA_KEYPAIR_JSON=\[' \
    . || true
)"

if [ -n "$matches" ]; then
  echo "security-scan: secret-like patterns found in:" >&2
  printf '%s\n' "$matches" >&2
  exit 1
fi

echo "security-scan: ok"
