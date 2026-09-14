#!/usr/bin/env sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
tmp_root="${TMPDIR:-/tmp}/solana-tee-guard-tests.$$"

cleanup() {
  rm -rf "$tmp_root"
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$tmp_root"

new_repo() {
  name="$1"
  repo="$tmp_root/$name"
  mkdir -p "$repo/scripts"
  cp "$repo_root/scripts/security-scan.sh" "$repo/scripts/security-scan.sh"
  cp "$repo_root/scripts/check-publish-history.sh" "$repo/scripts/check-publish-history.sh"
  (
    cd "$repo"
    git init -q
    git config user.email guard-tests@example.invalid
    git config user.name "Guard Tests"
  )
  printf '%s\n' "$repo"
}

expect_pass() {
  label="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    printf 'ok: %s\n' "$label"
  else
    printf 'not ok: %s\n' "$label" >&2
    "$@" >&2 || true
    exit 1
  fi
}

expect_fail() {
  label="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    printf 'not ok: %s unexpectedly passed\n' "$label" >&2
    exit 1
  fi
  printf 'ok: %s\n' "$label"
}

repo="$(new_repo clean)"
(
  cd "$repo"
  mkdir -p clawd-tee-gateway src
  printf 'UPSTREAM_API_KEY=\nADMIN_TOKEN=\n' > clawd-tee-gateway/.env.example
  printf 'pub fn main() {}\n' > src/lib.rs
  git add .
  git commit -qm "clean fixture"
)
expect_pass "clean repo passes working-tree scan" sh "$repo/scripts/security-scan.sh"
expect_pass "clean repo passes history scan" sh "$repo/scripts/check-publish-history.sh"

repo="$(new_repo untracked-local-secrets)"
(
  cd "$repo"
  mkdir -p .keys src
  printf 'not real key material\n' > .keys/devnet-payer.json
  printf 'pub fn main() {}\n' > src/lib.rs
  git add src scripts
  git commit -qm "safe tracked files"
)
expect_pass "untracked ignored secret directory does not fail working-tree scan" sh "$repo/scripts/security-scan.sh"

repo="$(new_repo tracked-secret-path)"
(
  cd "$repo"
  mkdir -p .keys
  printf 'not real key material\n' > .keys/devnet-payer.json
  git add -f .keys/devnet-payer.json scripts
)
expect_fail "tracked .keys path fails working-tree scan" sh "$repo/scripts/security-scan.sh"

repo="$(new_repo secret-like-source)"
(
  cd "$repo"
  mkdir -p src
  printf 'const token = "%s%s";\n' "sk-" "FAKEFAKEFAKEFAKEFAKEFAKE" > src/config.ts
  git add src scripts
)
expect_fail "secret-like source content fails working-tree scan" sh "$repo/scripts/security-scan.sh"

repo="$(new_repo contaminated-history)"
(
  cd "$repo"
  mkdir -p .keys src
  printf 'not real key material\n' > .keys/devnet-payer.json
  printf 'pub fn main() {}\n' > src/lib.rs
  git add -f .keys/devnet-payer.json src scripts
  git commit -qm "accidentally add local key"
  git rm -q .keys/devnet-payer.json
  git commit -qm "remove local key from head"
)
expect_pass "history fixture head passes working-tree scan" sh "$repo/scripts/security-scan.sh"
expect_fail "history fixture fails publish-history scan" sh "$repo/scripts/check-publish-history.sh"

printf 'security guard tests: ok\n'
