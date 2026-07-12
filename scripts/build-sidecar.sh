#!/usr/bin/env bash
# Build the LlamaChat Python sidecar into a single self-contained binary named
# `fitllm-sidecar` (no system Python needed at runtime), then stage it where
# Tauri's bundler expects an external binary.
#
# Usage (from anywhere):
#     scripts/build-sidecar.sh
#
# Prereqs: a Python 3.11+ environment with the pinned build deps installed:
#     python3 -m venv .venv-build && . .venv-build/bin/activate
#     pip install -r scripts/requirements-sidecar.txt
#
# Output:
#   dist/fitllm-sidecar                         (raw PyInstaller onefile binary)
#   src-tauri/binaries/fitllm-sidecar-<triple>  (Tauri externalBin naming)
#
# Tauri v2 requires external binaries to carry the *target triple* suffix on
# disk (e.g. fitllm-sidecar-aarch64-apple-darwin); the suffix is stripped when
# the binary is copied into the app bundle. tauri.conf.json references the base
# name "binaries/fitllm-sidecar".
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SPEC="${SCRIPT_DIR}/fitllm-sidecar.spec"
BIN_NAME="fitllm-sidecar"
DEST_DIR="${REPO_ROOT}/src-tauri/binaries"

cd "${REPO_ROOT}"

# --- sanity checks --------------------------------------------------------
if ! command -v pyinstaller >/dev/null 2>&1; then
  echo "error: pyinstaller not found. Install build deps first:" >&2
  echo "  pip install -r scripts/requirements-sidecar.txt" >&2
  exit 1
fi

# --- determine the Tauri target triple ------------------------------------
# Prefer FITLLM_SIDECAR_TRIPLE, else ask rustc for the host triple.
if [[ -n "${FITLLM_SIDECAR_TRIPLE:-}" ]]; then
  TRIPLE="${FITLLM_SIDECAR_TRIPLE}"
elif command -v rustc >/dev/null 2>&1; then
  TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
else
  echo "error: cannot determine target triple (no rustc, no FITLLM_SIDECAR_TRIPLE)" >&2
  exit 1
fi

echo ">> Freezing sidecar with PyInstaller (spec: ${SPEC})"
pyinstaller --clean --noconfirm "${SPEC}"

RAW_BIN="${REPO_ROOT}/dist/${BIN_NAME}"
if [[ ! -f "${RAW_BIN}" ]]; then
  echo "error: expected onefile binary at ${RAW_BIN} was not produced" >&2
  exit 1
fi

# --- stage for Tauri bundling --------------------------------------------
mkdir -p "${DEST_DIR}"
STAGED="${DEST_DIR}/${BIN_NAME}-${TRIPLE}"
cp -f "${RAW_BIN}" "${STAGED}"
chmod +x "${STAGED}"

echo ">> Sidecar binary staged for Tauri:"
echo "   ${STAGED}"
echo ">> tauri.conf.json bundle.externalBin references: binaries/${BIN_NAME}"
echo ">> Quick smoke test:"
"${RAW_BIN}" list-adapters || true
