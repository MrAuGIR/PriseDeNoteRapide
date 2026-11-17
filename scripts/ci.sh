#!/usr/bin/env sh
# Reproduit en local les vérifications de la CI GitHub (.github/workflows/ci.yml).
# Usage : ./scripts/ci.sh
# Prérequis couverture : cargo install cargo-llvm-cov && rustup component add llvm-tools-preview
set -eu

cd "$(dirname "$0")/.."

SEUIL_COUVERTURE="${SEUIL_COUVERTURE:-70}"

echo "==> Formatage"
cargo fmt --all --check

echo "==> Clippy"
cargo clippy --all-targets --all-features --locked -- -D warnings

echo "==> Tests"
cargo test --all-targets --locked

echo "==> Couverture (seuil ${SEUIL_COUVERTURE} % de lignes)"
if cargo llvm-cov --version >/dev/null 2>&1; then
    cargo llvm-cov --locked --summary-only --fail-under-lines "$SEUIL_COUVERTURE"
else
    echo "    cargo-llvm-cov absent : étape ignorée (cargo install cargo-llvm-cov)" >&2
fi

echo "==> OK"
