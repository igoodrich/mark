//! Reads the current Omarchy theme and turns it into CSS custom properties.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Where Omarchy keeps the active theme. A theme switch replaces this
/// directory's contents, so Mark watches it rather than a single file.
pub fn state_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state/omarchy/current"))
}

/// Builds the `:root { --… }` block from `theme/colors.toml`, falling back to
/// a plain dark palette with Omarchy's default yellow accent when it's missing.
pub fn css(state_dir: Option<&Path>, radius: u32) -> String {
    let colors = state_dir
        .and_then(|d| std::fs::read_to_string(d.join("theme/colors.toml")).ok())
        .and_then(|s| toml::from_str::<HashMap<String, toml::Value>>(&s).ok())
        .unwrap_or_default();
    let get = |key: &str, fallback: &str| -> String {
        colors.get(key).and_then(|v| v.as_str()).filter(|v| is_color(v)).unwrap_or(fallback).to_string()
    };
    let bg = get("background", "#121212");
    let vars = [
        ("bg", bg.clone()),
        ("bg-alt", get("lighter_background", "#1e1e1e")),
        ("fg", get("foreground", "#d8d8d8")),
        ("fg-strong", get("bright_foreground", "#ffffff")),
        ("fg-dim", get("light_foreground", "#9a9a9a")),
        ("accent", get("accent", "#FFD60A")),
        ("muted", get("muted", "#3a3a3a")),
        ("selection", get("selection", "#3a3a3a")),
        ("code", get("cyan", "#8cbfb8")),
        ("red", get("red", "#f7768e")),
        ("green", get("green", "#9ece6a")),
        ("yellow", get("yellow", "#e0af68")),
        ("blue", get("blue", "#7aa2f7")),
        ("magenta", get("magenta", "#bb9af7")),
    ];
    let scheme = match colors.get("mode").and_then(|v| v.as_str()) {
        Some("light") => "light",
        _ => "dark",
    };
    let mut out = format!(":root{{color-scheme:{scheme};--radius:{radius}px;");
    for (name, value) in vars {
        out.push_str(&format!("--{name}:{value};"));
    }
    out.push('}');
    out
}

/// Hyprland's window rounding, so code blocks and tables match the window
/// corners. Omarchy's default is 0 (sharp).
pub fn hypr_radius() -> u32 {
    std::process::Command::new("hyprctl")
        .args(["getoption", "decoration:rounding"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next()?.strip_prefix("int: ")?.trim().parse().ok())
        .unwrap_or(0)
}

/// Theme files are user-editable; only let plain color values into the CSS.
fn is_color(v: &str) -> bool {
    v.starts_with('#') && v.len() <= 9 && v[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_theme_colors() {
        let dir = std::env::temp_dir().join(format!("mark-theme-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("theme")).unwrap();
        std::fs::write(dir.join("theme/colors.toml"), "mode = \"light\"\naccent = \"#faa968\"\nbackground = \"red;}\"\n").unwrap();
        let css = css(Some(&dir), 8);
        std::fs::remove_dir_all(&dir).ok();
        assert!(css.contains("--accent:#faa968;"));
        assert!(css.contains("color-scheme:light"));
        assert!(css.contains("--radius:8px"));
        assert!(css.contains("--bg:#121212;"), "invalid colors fall back: {css}");
    }

    #[test]
    fn falls_back_without_omarchy() {
        assert!(css(None, 0).contains("--accent:#FFD60A;"));
    }
}
