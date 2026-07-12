//! Brand art for the TUI: the LlamaChat logo rendered from the real brand mark
//! (`llamachat-brand/icon-1024.png`) as half-block terminal art. Generated, not
//! hand-drawn — every row is padded to the same width so the block stays aligned
//! when the renderer centers it. Draw each row in the brand color.

pub const LOGO: [&str; 11] = [
    "    ▄██▄     ▄██▄  ",
    "    █  █▄▄▄▄▄█▀ █  ",
    "    █▄▄█▀   ▀█▄▄█  ",
    "  ▄█▀▀          ▀█▄",
    "  █▀  ▄  ▄▄▄  ▄  ▀█",
    "  ██ ▀▀█▀ ▄ ▀█▀▀ ██",
    "  ██   ▀▄▄▀▄▄█   ██",
    "  ██     ▀▀▀     ██",
    "  ▀█             █▀",
    "  ▄█             █▄",
    "  ██             ██",
];

pub const LOGO_SM: [&str; 8] = [
    "   ▄██    ██▄ ",
    "   █▄██▀▀██▄█ ",
    "  ▄█▀▀    ▀▀█▄",
    "  █ ▄▄▄▄▄▄▄▄ █",
    "  █▀ ▀▄  ▄▀ ██",
    "  █    ▀▀    █",
    "  ██        ██",
    "  █          █",
];
