//! CSS Color Level 4 子集：颜色解析。
//!
//! 当前支持：
//! - 命名颜色（CSS Color 4 §10 扩展颜色关键字表的常用子集）
//! - 十六进制：`#rgb` / `#rgba` / `#rrggbb` / `#rrggbbaa`
//!
//! 推迟（B-2 或后续）：
//! - `rgb()` / `rgba()` / `hsl()` / `hsla()` 函数
//! - `currentColor` / system colors
//! - `oklab()` / `lab()` / `color()` 等

use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;

/// RGBA 颜色（8-bit per channel，非预乘）。
///
/// `a = 0` 表示完全透明，`a = 255` 表示完全不透明。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    /// Red 通道（0-255）。
    pub r: u8,
    /// Green 通道（0-255）。
    pub g: u8,
    /// Blue 通道（0-255）。
    pub b: u8,
    /// Alpha 通道（0-255，255 = 不透明）。
    pub a: u8,
}

impl Color {
    /// 完全透明（`transparent`）。
    pub const TRANSPARENT: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// 黑色（`black` / `#000`）。
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    /// 白色（`white` / `#fff`）。
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    /// 从 RGB 构造不透明颜色。
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// 从 RGBA 构造。
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// 是否完全透明。
    pub fn is_transparent(self) -> bool {
        self.a == 0
    }
}

/// 解析 CSS 颜色值。
///
/// 输入为 cascade 输出的 [`ComputedValue`](muskitty_cascade::ComputedValue)
/// 内部的 component value 列表。返回 `None` 表示无法识别（调用方按
/// 初始值 `transparent` 处理）。
///
/// 支持：
/// - 单个 ident：命名颜色（`red`、`blue`、`transparent` 等）
/// - 单个 hash token：`#rgb` / `#rgba` / `#rrggbb` / `#rrggbbaa`
pub fn parse_color(values: &[ComponentValue]) -> Option<Color> {
    if values.is_empty() {
        return None;
    }
    // 仅取首个 component value；多值（如 `rgb(1,2,3)` 的函数形式）推迟。
    match &values[0] {
        ComponentValue::PreservedToken(Token::Ident(name)) => parse_named_color(name),
        ComponentValue::PreservedToken(Token::Hash(hex, _hash_type)) => parse_hex_color(hex),
        _ => None,
    }
}

/// 解析命名颜色。大小写不敏感。
///
/// 覆盖 CSS Color 4 §10 常用子集（16 个标准 HTML 颜色 + `transparent`）。
/// 完整的 148 个 X11 颜色推迟到需要时再补。
pub fn parse_named_color(name: &str) -> Option<Color> {
    // 大小写不敏感匹配
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "transparent" => Some(Color::TRANSPARENT),
        "black" => Some(Color::rgb(0, 0, 0)),
        "white" => Some(Color::rgb(255, 255, 255)),
        "red" => Some(Color::rgb(255, 0, 0)),
        "green" => Some(Color::rgb(0, 128, 0)), // CSS named "green" = #008000
        "blue" => Some(Color::rgb(0, 0, 255)),
        "yellow" => Some(Color::rgb(255, 255, 0)),
        "cyan" | "aqua" => Some(Color::rgb(0, 255, 255)),
        "magenta" | "fuchsia" => Some(Color::rgb(255, 0, 255)),
        "gray" | "grey" => Some(Color::rgb(128, 128, 128)),
        "silver" => Some(Color::rgb(192, 192, 192)),
        "lime" => Some(Color::rgb(0, 255, 0)),
        "maroon" => Some(Color::rgb(128, 0, 0)),
        "navy" => Some(Color::rgb(0, 0, 128)),
        "olive" => Some(Color::rgb(128, 128, 0)),
        "purple" => Some(Color::rgb(128, 0, 128)),
        "teal" => Some(Color::rgb(0, 128, 128)),
        _ => None,
    }
}

/// 解析十六进制颜色：`#rgb` / `#rgba` / `#rrggbb` / `#rrggbbaa`。
///
/// 长度 3/4 时每通道扩展为双倍（如 `#abc` → `#aabbcc`）。
/// 长度 8 时后两字节为 alpha。
pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        3 => {
            // #rgb → #rrggbb
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some(Color::rgb(r, g, b))
        }
        4 => {
            // #rgba → #rrggbbaa
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
            Some(Color::rgba(r, g, b, a))
        }
        6 => {
            // #rrggbb
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::rgb(r, g, b))
        }
        8 => {
            // #rrggbbaa
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Color::rgba(r, g, b, a))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_colors_basic() {
        assert_eq!(parse_named_color("red"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(parse_named_color("blue"), Some(Color::rgb(0, 0, 255)));
        assert_eq!(parse_named_color("transparent"), Some(Color::TRANSPARENT));
        assert!(parse_named_color("transparent").unwrap().is_transparent());
    }

    #[test]
    fn named_colors_case_insensitive() {
        assert_eq!(parse_named_color("RED"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(parse_named_color("Blue"), Some(Color::rgb(0, 0, 255)));
    }

    #[test]
    fn named_colors_aliases() {
        assert_eq!(parse_named_color("aqua"), parse_named_color("cyan"));
        assert_eq!(parse_named_color("fuchsia"), parse_named_color("magenta"));
        assert_eq!(parse_named_color("gray"), parse_named_color("grey"));
    }

    #[test]
    fn named_colors_unknown() {
        assert_eq!(parse_named_color("nonexistent"), None);
    }

    #[test]
    fn hex_3_digit() {
        assert_eq!(parse_hex_color("#f00"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(parse_hex_color("#abc"), Some(Color::rgb(0xaa, 0xbb, 0xcc)));
    }

    #[test]
    fn hex_4_digit_with_alpha() {
        let c = parse_hex_color("#f00f").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
        let c = parse_hex_color("#f000").unwrap();
        assert_eq!(c.a, 0);
        assert!(c.is_transparent());
    }

    #[test]
    fn hex_6_digit() {
        assert_eq!(parse_hex_color("#ff0000"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(parse_hex_color("#00ff00"), Some(Color::rgb(0, 255, 0)));
        assert_eq!(
            parse_hex_color("#abcdef"),
            Some(Color::rgb(0xab, 0xcd, 0xef))
        );
    }

    #[test]
    fn hex_8_digit_with_alpha() {
        let c = parse_hex_color("#ff0000ff").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
        let c = parse_hex_color("#ff000080").unwrap();
        assert_eq!(c.a, 0x80);
    }

    #[test]
    fn hex_without_hash_prefix() {
        // parse_hex_color 接受带或不带 # 的形式（因为 tokenizer 的 Hash token
        // 已去掉 # 前缀，只留 hex 部分）
        assert_eq!(parse_hex_color("f00"), Some(Color::rgb(255, 0, 0)));
    }

    #[test]
    fn hex_invalid_length() {
        assert_eq!(parse_hex_color("#ff"), None);
        assert_eq!(parse_hex_color("#fffff"), None);
        assert_eq!(parse_hex_color("#fffffffff"), None);
    }

    #[test]
    fn hex_invalid_chars() {
        assert_eq!(parse_hex_color("#gggggg"), None);
    }

    #[test]
    fn color_constants() {
        assert_eq!(Color::BLACK, Color::rgb(0, 0, 0));
        assert_eq!(Color::WHITE, Color::rgb(255, 255, 255));
        assert!(Color::TRANSPARENT.is_transparent());
        assert!(!Color::BLACK.is_transparent());
    }
}
