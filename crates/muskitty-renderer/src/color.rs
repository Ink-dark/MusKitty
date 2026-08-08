//! CSS Color Level 4 子集：颜色解析。
//!
//! 当前支持：
//! - 命名颜色（CSS Color 4 §10 扩展颜色关键字表的常用子集）
//! - 十六进制：`#rgb` / `#rgba` / `#rrggbb` / `#rrggbbaa`
//! - `rgb()` / `rgba()` 函数（CSS Color 4 §6，legacy + space 语法）
//!
//! 推迟（后续）：
//! - `hsl()` / `hsla()` 函数
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
/// - `rgb()` / `rgba()` 函数：legacy（`rgb(255, 0, 0)`）与 space
///   （`rgb(255 0 0)`）语法，alpha 可选（`/ 0.5` 或第 4 参数）
pub fn parse_color(values: &[ComponentValue]) -> Option<Color> {
    if values.is_empty() {
        return None;
    }
    match &values[0] {
        ComponentValue::PreservedToken(Token::Ident(name)) => parse_named_color(name),
        ComponentValue::PreservedToken(Token::Hash(hex, _hash_type)) => parse_hex_color(hex),
        ComponentValue::Function(func) => parse_color_function(&func.name, &func.value),
        _ => None,
    }
}

/// 解析 `rgb()` / `rgba()` 函数。
///
/// 支持的语法（CSS Color 4 §6）：
/// - `rgb(255, 0, 0)` / `rgba(255, 0, 0, 0.5)` — legacy comma 语法
/// - `rgb(255 0 0)` / `rgb(255 0 0 / 0.5)` — space 语法
/// - 通道值接受 `<number>`（0-255）或 `<percentage>`（0%-100%）
/// - alpha 接受 `<number>`（0-1）或 `<percentage>`（0%-100%）
fn parse_color_function(name: &str, args: &[ComponentValue]) -> Option<Color> {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "rgb" | "rgba" => parse_rgb(args),
        _ => None,
    }
}

/// 解析 rgb/rgba 参数列表。
fn parse_rgb(args: &[ComponentValue]) -> Option<Color> {
    // 提取所有数值 token（跳过逗号、空格、斜杠分隔符）
    let mut numbers: Vec<f64> = Vec::new();
    let mut alpha: Option<f64> = None;
    let mut slash_seen = false;

    for cv in args {
        match cv {
            ComponentValue::PreservedToken(Token::Number(n)) => {
                if slash_seen {
                    alpha = Some(n.value);
                } else {
                    numbers.push(n.value);
                }
            }
            ComponentValue::PreservedToken(Token::Percentage(p)) => {
                if slash_seen {
                    alpha = Some(p.value / 100.0);
                } else if numbers.len() < 3 {
                    // 0%-100% → 0-255
                    numbers.push(p.value / 100.0 * 255.0);
                }
            }
            ComponentValue::PreservedToken(Token::Comma)
            | ComponentValue::PreservedToken(Token::Whitespace) => {
                // 分隔符，跳过
            }
            ComponentValue::PreservedToken(Token::Delim('/')) => {
                // CSS Color 4 space 语法的 alpha 分隔符
                slash_seen = true;
            }
            _ => {}
        }
    }

    if numbers.len() < 3 {
        return None;
    }

    let r = clamp_channel(numbers[0]);
    let g = clamp_channel(numbers[1]);
    let b = clamp_channel(numbers[2]);
    let a = match alpha {
        Some(a) => clamp_alpha(a),
        None => {
            // rgba legacy 语法：第 4 个数值参数为 alpha
            if numbers.len() >= 4 {
                clamp_alpha(numbers[3])
            } else {
                255
            }
        }
    };

    Some(Color::rgba(r, g, b, a))
}

/// 将通道浮点值钳制到 0-255 u8。
fn clamp_channel(v: f64) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        v.round() as u8
    }
}

/// 将 alpha 浮点值（0.0-1.0）钳制到 0-255 u8。
fn clamp_alpha(v: f64) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 1.0 {
        255
    } else {
        (v * 255.0).round() as u8
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
    // P1-10：非 ASCII（如 `#aä`，`ä` 为多字节 UTF-8）或含非十六进制字符时
    // 直接返回 None。下方 `&hex[a..b]` 是字节切片，若输入含多字节字符会
    // 在非 char boundary 处 panic；此处先行校验全部字符为 ASCII 十六进制。
    if !hex.is_ascii() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
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
    fn hex_non_ascii_does_not_panic() {
        // P1-10：`#aä`（ä 为 2 字节 UTF-8）若走字节切片会 panic
        // "byte index is not a char boundary"。非 ASCII / 非十六进制必须
        // 返回 None，绝不 panic。
        assert_eq!(parse_hex_color("#aä"), None);
        assert_eq!(parse_hex_color("#aaä"), None);
        assert_eq!(parse_hex_color("#äa"), None);
    }

    #[test]
    fn color_constants() {
        assert_eq!(Color::BLACK, Color::rgb(0, 0, 0));
        assert_eq!(Color::WHITE, Color::rgb(255, 255, 255));
        assert!(Color::TRANSPARENT.is_transparent());
        assert!(!Color::BLACK.is_transparent());
    }

    // —— rgb() / rgba() 函数测试 ——

    fn parse_color_str(s: &str) -> Option<Color> {
        use muskitty_css::parser::parse_a_list_of_component_values;
        let cvs = parse_a_list_of_component_values(s);
        parse_color(&cvs)
    }

    #[test]
    fn rgb_legacy_comma() {
        assert_eq!(
            parse_color_str("rgb(255, 0, 0)"),
            Some(Color::rgb(255, 0, 0))
        );
        assert_eq!(
            parse_color_str("rgb(0, 255, 0)"),
            Some(Color::rgb(0, 255, 0))
        );
        assert_eq!(
            parse_color_str("rgb(128, 64, 32)"),
            Some(Color::rgb(128, 64, 32))
        );
    }

    #[test]
    fn rgb_space_syntax() {
        assert_eq!(parse_color_str("rgb(255 0 0)"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(
            parse_color_str("rgb(10 20 30)"),
            Some(Color::rgb(10, 20, 30))
        );
    }

    #[test]
    fn rgb_with_percentage() {
        assert_eq!(
            parse_color_str("rgb(100%, 0%, 0%)"),
            Some(Color::rgb(255, 0, 0))
        );
        assert_eq!(
            parse_color_str("rgb(50%, 50%, 50%)"),
            Some(Color::rgb(128, 128, 128))
        );
    }

    #[test]
    fn rgba_legacy_comma_with_alpha() {
        let c = parse_color_str("rgba(255, 0, 0, 0.5)").unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 0);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 128); // 0.5 * 255 ≈ 128
    }

    #[test]
    fn rgb_space_syntax_with_alpha() {
        let c = parse_color_str("rgb(255 0 0 / 0.5)").unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.a, 128);
    }

    #[test]
    fn rgb_alpha_percentage() {
        let c = parse_color_str("rgb(255 0 0 / 50%)").unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.a, 128);
    }

    #[test]
    fn rgb_clamping() {
        // 超出范围的值被钳制
        assert_eq!(
            parse_color_str("rgb(300, -10, 0)"),
            Some(Color::rgb(255, 0, 0))
        );
    }

    #[test]
    fn rgb_case_insensitive() {
        assert_eq!(
            parse_color_str("RGB(255, 0, 0)"),
            Some(Color::rgb(255, 0, 0))
        );
        assert_eq!(
            parse_color_str("Rgba(255, 0, 0, 1.0)"),
            Some(Color::rgb(255, 0, 0))
        );
    }

    #[test]
    fn rgb_too_few_args() {
        assert_eq!(parse_color_str("rgb(255, 0)"), None);
        assert_eq!(parse_color_str("rgb(255)"), None);
    }
}
