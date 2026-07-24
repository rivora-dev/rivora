//! Semantic color roles.
#![allow(dead_code)]

use ratatui::style::Color;

/// Semantic colors (avoid relying on color alone for meaning).
#[derive(Debug, Clone, Copy)]
pub struct SemanticColor {
    pub primary: Color,
    pub muted: Color,
    pub accent: Color,
    pub focused: Color,
    pub selected: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub border: Color,
    pub disabled: Color,
}

/// Default palette; respects NO_COLOR by falling back to Reset-ish defaults.
pub fn palette() -> SemanticColor {
    if std::env::var_os("NO_COLOR").is_some() {
        return SemanticColor {
            primary: Color::White,
            muted: Color::Gray,
            accent: Color::White,
            focused: Color::White,
            selected: Color::White,
            success: Color::White,
            warning: Color::White,
            error: Color::White,
            border: Color::Gray,
            disabled: Color::DarkGray,
        };
    }
    SemanticColor {
        primary: Color::White,
        muted: Color::DarkGray,
        accent: Color::Cyan,
        focused: Color::LightCyan,
        selected: Color::Yellow,
        success: Color::Green,
        warning: Color::Yellow,
        error: Color::Red,
        border: Color::DarkGray,
        disabled: Color::DarkGray,
    }
}
