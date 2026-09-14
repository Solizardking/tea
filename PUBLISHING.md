# Publishing Checklist

This repository is not ready for a public GitHub push until both the current
index and the full Git history are clean.

## Required Before First Public Push

1. Rotate every key that has ever been committed or staged, including local
   Solana fee-payer keys and TEE signing keys.
2. Run `sh scripts/security-scan.sh`.
3. Run `sh scripts/test-security-guards.sh`.
4. Run `sh scripts/check-publish-history.sh`.
5. If the history check fails, replace or rewrite history before pushing.
6. Re-run all three scripts after the history rewrite.
7. Run `cargo fmt --check`, `cargo check -p solana-redpill-verifier`,
   `cargo check -p clawd-tee-gateway`, and `npm run build` in
   `clients/typescript`.
8. Review `SECURITY.md` and configure GitHub secret scanning, branch protection,
   and required CI checks on the public repository.

## Current Repo Notes

`.keys/`, `target/`, `node_modules/`, `dist/`, `.env`, PEM files, wallet JSON,
and keypair JSON files must never be tracked. Local copies may exist on disk for
development, but they must stay ignored and outside the public commit history.

If this local repository already has forbidden paths in `git rev-list --objects
--all`, do not push it as-is. Start a fresh sanitized repository or rewrite the
nested repository history, then force-push only after affected credentials have
been rotated.

## Runtime Defaults For Public Gateways

- `ALLOW_CLIENT_AUTHORIZATION=false`
- `ALLOW_INSECURE_UPSTREAM=false`
- `SAS_PROOF_THRESHOLD=0` until Solana anchoring and `ADMIN_TOKEN` are configured
- `VERIFICATION_LEVEL=0` until the native secp256k1 transaction path is verified
- `CORS_ALLOWED_ORIGIN` unset unless a browser client requires a concrete origin
