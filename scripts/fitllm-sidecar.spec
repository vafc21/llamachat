# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for the FitLLM sidecar.

Freezes the ``fitllm_sidecar`` package into a single self-contained binary
named ``fitllm-sidecar`` (no system Python required at runtime). This lets the
macOS .dmg (and other bundles) ship the sidecar as a Tauri external binary.

Build (from the repo root)::

    scripts/build-sidecar.sh

or directly::

    pyinstaller --clean --noconfirm scripts/fitllm-sidecar.spec

The entry point is ``scripts/fitllm_sidecar_entry.py``, which imports and runs
``fitllm_sidecar.__main__:main`` — equivalent to ``python -m fitllm_sidecar``.
"""

import os

# ``SPECPATH`` is the directory containing this spec file (scripts/), injected
# by PyInstaller. Derive the repo root and the sidecar source tree from it so
# the spec works regardless of the current working directory.
REPO_ROOT = os.path.abspath(os.path.join(SPECPATH, ".."))
SIDECAR_SRC = os.path.join(REPO_ROOT, "sidecar", "src")
ENTRY = os.path.join(SPECPATH, "fitllm_sidecar_entry.py")


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
        "fitllm_sidecar",
        "fitllm_sidecar.__main__",
        "fitllm_sidecar.benchmark",
        "fitllm_sidecar.server",
        "fitllm_sidecar.sysmon",
        "fitllm_sidecar.adapters",
        "fitllm_sidecar.adapters.base",
        "fitllm_sidecar.adapters.ollama",
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
    name="fitllm-sidecar",
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
