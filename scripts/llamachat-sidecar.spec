# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for the LlamaChat sidecar.

Freezes the ``llamachat_sidecar`` package into a single self-contained binary
named ``llamachat-sidecar`` (no system Python required at runtime). This lets the
macOS .dmg (and other bundles) ship the sidecar as a Tauri external binary.

Build (from the repo root)::

    scripts/build-sidecar.sh

or directly::

    pyinstaller --clean --noconfirm scripts/llamachat-sidecar.spec

The entry point is ``scripts/llamachat_sidecar_entry.py``, which imports and runs
``llamachat_sidecar.__main__:main`` — equivalent to ``python -m llamachat_sidecar``.
"""

import os

# ``SPECPATH`` is the directory containing this spec file (scripts/), injected
# by PyInstaller. Derive the repo root and the sidecar source tree from it so
# the spec works regardless of the current working directory.
REPO_ROOT = os.path.abspath(os.path.join(SPECPATH, ".."))
SIDECAR_SRC = os.path.join(REPO_ROOT, "sidecar", "src")
ENTRY = os.path.join(SPECPATH, "llamachat_sidecar_entry.py")


block_cipher = None


a = Analysis(
    [ENTRY],
    pathex=[SIDECAR_SRC],
    binaries=[],
    datas=[],
    # Imports are static (adapters/__init__ imports ollama), but list the
    # runtime deps and submodules explicitly so a stripped analysis never
    # drops them.
    hiddenimports=[
        "llamachat_sidecar",
        "llamachat_sidecar.__main__",
        "llamachat_sidecar.benchmark",
        "llamachat_sidecar.server",
        "llamachat_sidecar.sysmon",
        "llamachat_sidecar.adapters",
        "llamachat_sidecar.adapters.base",
        "llamachat_sidecar.adapters.ollama",
        "requests",
        "psutil",
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name="llamachat-sidecar",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,  # the sidecar is a CLI / stdio RPC process
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,  # set FITLLM_SIDECAR_ARCH / --target-arch for universal2
    codesign_identity=None,
    entitlements_file=None,
)
