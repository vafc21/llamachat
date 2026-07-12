# Build the LlamaChat Python sidecar into a single self-contained binary
# (llamachat-sidecar.exe) on Windows, then stage it where Tauri's bundler expects
# an external binary.
#
# Usage (from anywhere):
#     pwsh scripts/build-sidecar.ps1        # or Windows PowerShell
#
# Prereqs: Python 3.11+ with the pinned build deps installed:
#     python -m venv .venv-build
#     .\.venv-build\Scripts\Activate.ps1
#     pip install -r scripts/requirements-sidecar.txt
#
# Output:
#   dist\llamachat-sidecar.exe                          (raw PyInstaller onefile)
#   src-tauri\binaries\llamachat-sidecar-<triple>.exe   (Tauri externalBin naming)
#
# Tauri v2 requires external binaries to carry the target-triple suffix on disk
# (e.g. llamachat-sidecar-x86_64-pc-windows-msvc.exe); the suffix is stripped when
# copied into the app. tauri.conf.json references the base "binaries/llamachat-sidecar".

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot  = Split-Path -Parent $ScriptDir
$Spec      = Join-Path $ScriptDir "llamachat-sidecar.spec"
$BinName   = "llamachat-sidecar"
$DestDir   = Join-Path $RepoRoot "src-tauri\binaries"

Set-Location $RepoRoot

# --- sanity checks --------------------------------------------------------
if (-not (Get-Command pyinstaller -ErrorAction SilentlyContinue)) {
    Write-Error "pyinstaller not found. Install build deps first: pip install -r scripts/requirements-sidecar.txt"
}

# --- determine the Tauri target triple ------------------------------------
# Prefer FITLLM_SIDECAR_TRIPLE, else ask rustc, else assume the MSVC host.
if ($env:FITLLM_SIDECAR_TRIPLE) {
    $Triple = $env:FITLLM_SIDECAR_TRIPLE
} elseif (Get-Command rustc -ErrorAction SilentlyContinue) {
    $Triple = ((rustc -vV | Select-String '^host: ') -replace 'host: ', '').Trim()
} else {
    $Triple = "x86_64-pc-windows-msvc"
}

Write-Host ">> Freezing sidecar with PyInstaller (spec: $Spec)"
# PyInstaller writes its INFO log to stderr. Under Windows PowerShell 5.1 with
# $ErrorActionPreference='Stop', the first stderr line is turned into a
# terminating error and aborts the build before PyInstaller does any work.
# Relax the preference around the native call and gate on the real exit code.
$prevEAP = $ErrorActionPreference
$ErrorActionPreference = "Continue"
pyinstaller --clean --noconfirm $Spec 2>&1 | ForEach-Object { "$_" }
$pyExit = $LASTEXITCODE
$ErrorActionPreference = $prevEAP
if ($pyExit -ne 0) {
    Write-Error "PyInstaller failed (exit code $pyExit)"
}

$RawBin = Join-Path $RepoRoot "dist\$BinName.exe"
if (-not (Test-Path $RawBin)) {
    Write-Error "expected onefile binary at $RawBin was not produced"
}

# --- stage for Tauri bundling --------------------------------------------
New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
$Staged = Join-Path $DestDir "$BinName-$Triple.exe"
Copy-Item -Force $RawBin $Staged

Write-Host ">> Sidecar binary staged for Tauri:"
Write-Host "   $Staged"
Write-Host ">> tauri.conf.json bundle.externalBin references: binaries/$BinName"
Write-Host ">> Quick smoke test:"
& $RawBin list-adapters
