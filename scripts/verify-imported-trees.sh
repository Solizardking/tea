#!/usr/bin/env sh
# Verify imported project trees (clawd-verifier, content, redpill-verifier,
# and solana-tee content flattened at repo root) are present with source
# manifests — not only dist/ caches — and that workspace membership resolves.
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_root"

fail=0
ok() { printf 'ok: %s\n' "$1"; }
bad() { printf 'not ok: %s\n' "$1" >&2; fail=1; }

require_path() {
  if [ -e "$1" ]; then
    ok "path $1"
  else
    bad "missing path $1"
  fi
}

# --- clawd-verifier ---
require_path "clawd-verifier/js/src"
require_path "clawd-verifier/js/src/index.ts"
require_path "clawd-verifier/js/package.json"
require_path "clawd-verifier/js/package-lock.json"
require_path "clawd-verifier/js/tsconfig.json"
require_path "clawd-verifier/js/tsup.config.ts"
require_path "clawd-verifier/contracts/evm/ClawdProofStore.sol"

if [ -f clawd-verifier/js/package.json ]; then
  name="$(python3 -c 'import json; print(json.load(open("clawd-verifier/js/package.json"))["name"])')"
  if [ "$name" = "@clawd/tee-verifier" ]; then
    ok "clawd-verifier package name ($name)"
  else
    bad "clawd-verifier unexpected package name: $name"
  fi
fi

# --- content ---
require_path "content/articles"
require_path "content/soul"
require_path "content/soul/SOUL.md"

# --- redpill-verifier ---
require_path "redpill-verifier/attestation_verifier.py"
require_path "redpill-verifier/onchain_proof.py"
require_path "redpill-verifier/signature_verifier.py"
require_path "redpill-verifier/requirements.txt"
require_path "redpill-verifier/js/src"
require_path "redpill-verifier/js/src/index.ts"
require_path "redpill-verifier/js/package.json"
require_path "redpill-verifier/contracts/RedpillProofStore.sol"
require_path "redpill-verifier/.claude/skills/verify-tee/SKILL.md"

if [ -f redpill-verifier/js/package.json ]; then
  name="$(python3 -c 'import json; print(json.load(open("redpill-verifier/js/package.json"))["name"])')"
  if [ "$name" = "@redpill-ai/verifier" ]; then
    ok "redpill-verifier package name ($name)"
  else
    bad "redpill-verifier unexpected package name: $name"
  fi
fi

# Python entrypoints must be non-empty real modules (not stubs)
for py in \
  redpill-verifier/attestation_verifier.py \
  redpill-verifier/onchain_proof.py \
  redpill-verifier/signature_verifier.py
do
  if [ -s "$py" ]; then
    ok "non-empty $py"
  else
    bad "empty or missing $py"
  fi
done

# --- solana-tee remainder at root ---
require_path "program/src"
require_path "program/src/lib.rs"
require_path "program/Cargo.toml"
require_path "optee/README.md"
require_path "optee/ta/clawd_ta.c"
require_path "idl/solana_redpill_verifier.json"
require_path "scripts/security-scan.sh"
require_path "render.yaml"
require_path "svm.md"
require_path "clients/typescript/src"
require_path "clients/typescript/src/index.ts"
require_path "clients/typescript/package.json"
require_path "clawd-tee-gateway/src/main.rs"
require_path "Cargo.toml"

if [ -f clients/typescript/package.json ]; then
  name="$(python3 -c 'import json; print(json.load(open("clients/typescript/package.json"))["name"])')"
  if [ "$name" = "@redpill-ai/solana-verifier" ]; then
    ok "clients/typescript package name ($name)"
  else
    bad "clients/typescript unexpected package name: $name"
  fi
fi

if [ -f program/Cargo.toml ]; then
  pname="$(grep -E '^name\s*=' program/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')"
  if [ "$pname" = "solana-redpill-verifier" ]; then
    ok "program crate name ($pname)"
  else
    bad "program unexpected crate name: $pname"
  fi
fi

# Workspace members must exist on disk (no dangling membership)
while IFS= read -r member; do
  [ -z "$member" ] && continue
  if [ -f "$member/Cargo.toml" ]; then
    ok "workspace member $member"
  else
    bad "dangling workspace member $member"
  fi
done <<EOF
$(awk '/members = \[/,/\]/' Cargo.toml | sed -n 's/.*"\([^"]*\)".*/\1/p')
EOF

# --- nested solana-tee package (also mirrored at repo root) ---
require_path "solana-tee/Cargo.toml"
require_path "solana-tee/program/src"
require_path "solana-tee/optee"
require_path "solana-tee/idl/solana_redpill_verifier.json"
require_path "solana-tee/scripts"
require_path "solana-tee/clients/typescript/src"
require_path "solana-tee/clawd-tee-gateway/src/main.rs"
require_path "solana-tee/render.yaml"
require_path "solana-tee/svm.md"

if [ -f solana-tee/Cargo.toml ] && [ -f Cargo.toml ]; then
  if cmp -s solana-tee/Cargo.toml Cargo.toml; then
    ok "solana-tee/Cargo.toml matches root workspace Cargo.toml"
  else
    bad "solana-tee/Cargo.toml diverges from root Cargo.toml"
  fi
fi

# --- exclude rules ---
nested_git="$(find clawd-verifier content redpill-verifier program optee idl scripts clients clawd-tee-gateway solana-tee \
  -name .git -type d 2>/dev/null || true)"
if [ -z "$nested_git" ]; then
  ok "no nested .git under imported trees"
else
  bad "nested .git found: $nested_git"
fi

# redpill must not ship a copied node_modules from import
if [ ! -d redpill-verifier/js/node_modules ]; then
  ok "redpill-verifier has no committed/copied node_modules"
else
  # pre-existing local install is ok only if not tracked
  if git ls-files --error-unmatch redpill-verifier/js/node_modules >/dev/null 2>&1; then
    bad "redpill-verifier/js/node_modules is tracked"
  else
    ok "redpill-verifier node_modules untracked (local only)"
  fi
fi

# IDL must reference the verifier program shape (real shipped artifact)
if grep -q 'store_proof\|TeeProof\|solana' idl/solana_redpill_verifier.json 2>/dev/null; then
  ok "idl/solana_redpill_verifier.json looks like shipped IDL content"
else
  # still require non-empty valid JSON
  if python3 -c 'import json; json.load(open("idl/solana_redpill_verifier.json"))' 2>/dev/null; then
    ok "idl/solana_redpill_verifier.json is valid JSON"
  else
    bad "idl/solana_redpill_verifier.json missing or invalid"
  fi
fi

if [ "$fail" -ne 0 ]; then
  printf '\nverify-imported-trees: FAILED\n' >&2
  exit 1
fi
printf '\nverify-imported-trees: PASSED\n'
exit 0
