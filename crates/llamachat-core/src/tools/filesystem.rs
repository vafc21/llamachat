//! Filesystem tool: read, write, and edit files.

use crate::tools::{Tool, ToolInfo, ToolParam, ToolResult, ToolSafety, ToolLimits};
use std::fs;
use std::path::Path;

pub struct FilesystemTool {
    limits: ToolLimits,
}

impl FilesystemTool {
    pub fn new(limits: ToolLimits) -> Self {
        FilesystemTool { limits }
    }
}

impl Tool for FilesystemTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "file".into(),
            description: "Read or write files on the filesystem. Use 'read' to view a file's contents, 'write' to create or overwrite a file, 'edit' for targeted text replacements.".into(),
            safety: ToolSafety::Write,
            parameters: vec![
                ToolParam {
                    name: "action".into(),
                    description: "One of: read, write, edit".into(),
                    required: true,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "path".into(),
                    description: "Absolute or relative file path.".into(),
                    required: true,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "content".into(),
                    description: "Content to write (required for write action).".into(),
                    required: false,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "old_text".into(),
                    description: "Text to find and replace (required for edit action).".into(),
                    required: false,
                    param_type: "string".into(),
                },
                ToolParam {
                    name: "new_text".into(),
                    description: "Replacement text (required for edit action).".into(),
                    required: false,
                    param_type: "string".into(),
                },
            ],
        }
    }

    fn execute(&self, args: serde_json::Value) -> Result<ToolResult, String> {
        let action = args["action"]
            .as_str()
            .ok_or("Missing required arg: action (read|write|edit)")?;
        let path_str = args["path"]
            .as_str()
            .ok_or("Missing required arg: path")?;

        let path = Path::new(path_str);

        match action {
            "read" => self.read(path),
            "write" => {
                let content = args["content"]
                    .as_str()
                    .ok_or("Missing required arg: content")?;
                self.write(path, content)
            }
            "edit" => {
                let old = args["old_text"]
                    .as_str()
                    .ok_or("Missing required arg: old_text")?;
                let new = args["new_text"]
                    .as_str()
                    .ok_or("Missing required arg: new_text")?;
                self.edit(path, old, new)
            }
            _ => Err(format!("Unknown file action: {}. Use read, write, or edit.", action)),
        }
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::Write
    }
}

impl FilesystemTool {
    fn read(&self, path: &Path) -> Result<ToolResult, String> {
        // Safety: don't read binary or system files
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let binary_exts = ["exe", "dll", "so", "dylib", "bin", "dat", "db", "sqlite",
                               "png", "jpg", "jpeg", "gif", "mp4", "mp3", "zip", "tar", "gz"];
            if binary_exts.contains(&ext) {
                return Ok(ToolResult {
                    ok: false,
                    output: None,
                    error: Some(format!("Binary file type .{} — use a different tool to inspect", ext)),
                    media: None,
                    elapsed_ms: 0,
                });
            }
        }

        let metadata = fs::metadata(path).map_err(|e| format!("Cannot access {}: {}", path.display(), e))?;

        if metadata.len() > self.limits.max_file_read as u64 {
            return Ok(ToolResult {
                ok: false,
                output: None,
                error: Some(format!(
                    "File too large: {} (limit: {}MB)",
                    metadata.len(),
                    self.limits.max_file_read / (1024 * 1024)
                )),
                media: None,
                elapsed_ms: 0,
            });
        }

        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

        let lines: Vec<&str> = contents.lines().collect();
        let preview = if lines.len() > 200 {
            let head: Vec<&str> = lines.iter().take(200).copied().collect();
            format!("{}\n... ({} lines total, showing first 200)", head.join("\n"), lines.len())
        } else {
            contents
        };

        Ok(ToolResult {
            ok: true,
            output: Some(preview),
            error: None,
            media: None,
            elapsed_ms: 0,
        })
    }

    fn write(&self, path: &Path, content: &str) -> Result<ToolResult, String> {
        if content.len() > self.limits.max_file_write {
            return Ok(ToolResult {
                ok: false,
                output: None,
                error: Some(format!(
                    "Content too large: {} (limit: {}MB)",
                    content.len(),
                    self.limits.max_file_write / (1024 * 1024)
                )),
                media: None,
                elapsed_ms: 0,
            });
        }

        // Create parent dirs if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Cannot create directory {}: {}", parent.display(), e))?;
            }
        }

        // Warn if overwriting
        let existed = path.exists();

        fs::write(path, content)
            .map_err(|e| format!("Cannot write {}: {}", path.display(), e))?;

        let size = content.len();
        Ok(ToolResult {
            ok: true,
            output: Some(if existed {
                format!("Overwrote {} ({} bytes)", path.display(), size)
            } else {
                format!("Created {} ({} bytes)", path.display(), size)
            }),
            error: None,
            media: None,
            elapsed_ms: 0,
        })
    }

    fn edit(&self, path: &Path, old_text: &str, new_text: &str) -> Result<ToolResult, String> {
        let original = fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

        if !original.contains(old_text) {
            return Ok(ToolResult {
                ok: false,
                output: None,
                error: Some("old_text not found in file".into()),
                media: None,
                elapsed_ms: 0,
            });
        }

        let count = original.matches(old_text).count();
        if count > 1 {
            return Ok(ToolResult {
                ok: false,
                output: None,
                error: Some(format!(
                    "old_text matched {} times — must be unique. Provide more context.",
                    count
                )),
                media: None,
                elapsed_ms: 0,
            });
        }

        let modified = original.replacen(old_text, new_text, 1);
        fs::write(path, &modified)
            .map_err(|e| format!("Cannot write {}: {}", path.display(), e))?;

        Ok(ToolResult {
            ok: true,
            output: Some(format!("Edited {} — 1 replacement", path.display())),
            error: None,
            media: None,
            elapsed_ms: 0,
        })
    }
}
