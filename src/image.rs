use base64::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 尝试从系统剪贴板获取图片数据 (返回: mime_type, base64_encoded_string)
pub fn get_clipboard_image() -> Result<(String, String), String> {
    // 1. Linux 下检测 xclip
    if let Ok(output) = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "TARGETS", "-o"])
        .output()
    {
        let targets = String::from_utf8_lossy(&output.stdout);
        if targets.contains("image/png") {
            if let Ok(img_out) = Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "image/png", "-o"])
                .output()
            {
                if !img_out.stdout.is_empty() {
                    let b64 = BASE64_STANDARD.encode(&img_out.stdout);
                    return Ok(("image/png".to_string(), b64));
                }
            }
        }
        if targets.contains("image/jpeg") || targets.contains("image/jpg") {
            if let Ok(img_out) = Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "image/jpeg", "-o"])
                .output()
            {
                if !img_out.stdout.is_empty() {
                    let b64 = BASE64_STANDARD.encode(&img_out.stdout);
                    return Ok(("image/jpeg".to_string(), b64));
                }
            }
        }
    }

    // 2. Linux 下检测 wl-paste (Wayland)
    if let Ok(output) = Command::new("wl-paste")
        .args(["--list-types"])
        .output()
    {
        let types = String::from_utf8_lossy(&output.stdout);
        if types.contains("image/png") {
            if let Ok(img_out) = Command::new("wl-paste")
                .args(["--type", "image/png"])
                .output()
            {
                if !img_out.stdout.is_empty() {
                    let b64 = BASE64_STANDARD.encode(&img_out.stdout);
                    return Ok(("image/png".to_string(), b64));
                }
            }
        }
    }

    // 3. 检查剪贴板中的纯文本是否为本地已有图片的文件路径
    if let Ok(output) = Command::new("xclip")
        .args(["-selection", "clipboard", "-o"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        let trimmed = text.trim().trim_matches('"').trim_matches('\'');
        let clean_path = trimmed.strip_prefix("file://").unwrap_or(trimmed);
        let p = Path::new(clean_path);
        if p.exists() && p.is_file() && is_image_path(p) {
            return load_image_file(p);
        }
    }

    // 4. macOS 检测 (osascript / pngpaste)
    #[cfg(target_os = "macos")]
    {
        if let Ok(img_out) = Command::new("pngpaste").arg("-").output() {
            if !img_out.stdout.is_empty() {
                let b64 = BASE64_STANDARD.encode(&img_out.stdout);
                return Ok(("image/png".to_string(), b64));
            }
        }
    }

    // 5. Windows 检测 (PowerShell Clipboard)
    #[cfg(target_os = "windows")]
    {
        let ps_cmd = "Add-Type -AssemblyName System.Windows.Forms; $img = [System.Windows.Forms.Clipboard]::GetImage(); if ($img) { $ms = New-Object System.IO.MemoryStream; $img.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); [System.Convert]::ToBase64String($ms.ToArray()); }";
        if let Ok(output) = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", ps_cmd])
            .output()
        {
            let b64 = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !b64.is_empty() {
                return Ok(("image/png".to_string(), b64));
            }
        }
    }

    Err("未在系统剪贴板中检测到有效图片数据。请截图复制后重试，或直接提供本地图片路径。".to_string())
}

/// 读取并编码本地图片文件为 Base64
pub fn load_image_file(path: &Path) -> Result<(String, String), String> {
    if !path.exists() {
        return Err(format!("图片文件不存在: '{}'", path.display()));
    }

    let bytes = fs::read(path).map_err(|e| format!("读取图片文件失败: {}", e))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    };

    let b64 = BASE64_STANDARD.encode(&bytes);
    Ok((mime.to_string(), b64))
}

/// 检查路径扩展名是否为常见图像格式
pub fn is_image_path(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp")
}

/// 从用户输入中智能探测是否直接嵌入或拖拽了本地图片路径
pub fn detect_image_in_prompt(prompt: &str) -> Option<(PathBuf, String)> {
    let parts: Vec<&str> = prompt.split_whitespace().collect();
    for (idx, part) in parts.iter().enumerate() {
        let clean = part.trim_matches('"').trim_matches('\'');
        let path = Path::new(clean);
        if path.is_file() && is_image_path(path) {
            let mut rest = parts.clone();
            rest.remove(idx);
            let remaining_prompt = rest.join(" ");
            return Some((path.to_path_buf(), remaining_prompt));
        }
    }
    None
}

/// 检查指定模型是否支持多模态视觉 (Vision / Image)
pub fn is_vision_model(model: &str) -> bool {
    let m = model.to_lowercase();
    m.contains("gemini")
        || m.contains("sol")
        || m.contains("terra")
        || m.contains("luna")
        || m.contains("gpt-5.6")
        || m.contains("gpt-5.5")
        || m.contains("kimi-k2.6")
        || m.contains("minimax")
        || m.contains("vision")
        || m.contains("image")
}

/// 建议最佳多模态视觉模型
pub fn recommended_vision_model() -> &'static str {
    "gemini-3.5-flash"
}
