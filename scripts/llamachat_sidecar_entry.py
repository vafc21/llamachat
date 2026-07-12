"""Frozen-binary entry point for the LlamaChat sidecar.

PyInstaller freezes a *script*, but the sidecar is a package invoked as
``python -m llamachat_sidecar``. Its ``__main__`` uses package-relative imports,
so it cannot be frozen as a bare top-level script. This tiny launcher imports
the package properly (as ``llamachat_sidecar.__main__``) and calls ``main()``,
which is exactly equivalent to ``python -m llamachat_sidecar``.

The resulting binary is named ``llamachat-sidecar`` and accepts the same argv as
``python -m llamachat_sidecar`` (e.g. ``llamachat-sidecar benchmark --adapter ollama
--model llama3.2:1b --tier full``).
"""

import sys

from llamachat_sidecar.__main__ import main

if __name__ == "__main__":
    sys.exit(main())
