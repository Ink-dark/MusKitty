//! 图像解码（BG-1，M-3 batch 5 前置）。
//!
//! `background-image` 的绘制前置：把字节解码为 RGBA 像素 + 尺寸。
//! 解码走 tiny-skia 内置的 [`tiny_skia::Pixmap::decode_png`]（png crate 已在
//! 依赖图内，不新增直接依赖）；JPEG/GIF/WebP 暂不支持（解码失败按"无背景图"
//! 处理，非致命，缺口记录于计划文档）。
//!
//! # ADR（外部依赖解耦）
//!
//! [`ImageBits`] 是本 crate 自有抽象类型（RGBA8 字节 + 逻辑尺寸）；
//! `tiny_skia::Pixmap` / `png` 类型不出现在任何 pub 签名——解码函数内部
//! 即转为自有类型，`decode_png` 返回的错误同样折叠为 `Option`。

/// 解码后的图像：RGBA8（每像素 4 字节，行优先）+ 宽高。
///
/// 尺寸单位为**像素**（图像的内在尺寸，CSS px 语义下按 1x 使用——
/// background-size / DPR 缩放暂不支持，按初始值 natural size 绘制）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageBits {
    /// RGBA8 像素数据，长度恒为 `width * height * 4`。
    pub data: Vec<u8>,
    /// 宽（px）。
    pub width: u32,
    /// 高（px）。
    pub height: u32,
}

/// 解码后总像素上限（审计 C-1 防线）。
///
/// 60000×60000 的大面积纯色 PNG 经 deflate 可压到远小于 chrome 层
/// `max_image_bytes`（16 MiB）的体积，但解码需按 IHDR 声明的宽高分配
/// `w*h*4` 字节（≈14.4 GB）——tiny-skia 解码路径直接分配，失败走 OOM
/// abort 而非返回错误。解码前必须在 IHDR 阶段拒绝超界尺寸。
pub const MAX_IMAGE_PIXELS: u64 = 1 << 26; // 67,108,864 px（解码后 ≤ 256 MiB RGBA）

/// 单边像素上限（配合 [`MAX_IMAGE_PIXELS`]，极端长宽比单独封边）。
pub const MAX_IMAGE_DIMENSION: u32 = 16384;

impl ImageBits {
    /// 解码 PNG 字节为 [`ImageBits`]；非法 PNG / 不支持的格式返回 `None`。
    ///
    /// tiny-skia 的 `decode_png` 已覆盖 8/16-bit、灰度/RGB/索引/带 alpha 全
    /// 变体并统一为 RGBA8；解码失败（含截断、非 PNG 魔数）一律 `None`，
    /// 调用方按"图像不可用 → 跳过背景图"处理（HTML/CSS 语义：资源加载
    /// 失败不阻塞渲染）。
    ///
    /// 解码前先读 IHDR 声明尺寸做上限检查（审计 C-1）：压缩后体积受
    /// chrome 层 `max_image_bytes` 约束，但声明尺寸不受其约束，超界
    /// （单边 > [`MAX_IMAGE_DIMENSION`] 或总像素 > [`MAX_IMAGE_PIXELS`]）
    /// 直接按"图像不可用"返回 `None`，不进入分配路径。
    #[cfg(feature = "backend-tiny-skia")]
    pub fn from_png(bytes: &[u8]) -> Option<Self> {
        if let Some((w, h)) = png_ihdr_dimensions(bytes) {
            if !dimensions_within_limits(w, h) {
                return None;
            }
        }
        let pixmap = tiny_skia::Pixmap::decode_png(bytes).ok()?;
        let width = pixmap.width();
        let height = pixmap.height();
        Some(Self {
            data: pixmap.take(),
            width,
            height,
        })
    }
}

/// 从 PNG 字节读 IHDR 声明的宽高（不解码、不分配像素）。
///
/// PNG 布局：8 字节签名 + 4 字节 chunk 长度 + `"IHDR"` + 13 字节数据
/// （width 4B 大端 / height 4B 大端 / …）。签名或 IHDR 缺失时返回
/// `None`（后续 `decode_png` 同样会失败，此处不重复报错语义）。
fn png_ihdr_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 24 || bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Some((width, height))
}

/// IHDR 尺寸是否在上限内（零尺寸同样拒绝——tiny-skia 对 0 尺寸返回
/// `None`，提前判定保持语义一致）。
fn dimensions_within_limits(width: u32, height: u32) -> bool {
    width != 0
        && height != 0
        && width <= MAX_IMAGE_DIMENSION
        && height <= MAX_IMAGE_DIMENSION
        && u64::from(width) * u64::from(height) <= MAX_IMAGE_PIXELS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2x2 红蓝格 PNG（测试内编码，不依赖外部 fixture）。
    fn sample_png() -> Vec<u8> {
        let mut pixmap = tiny_skia::Pixmap::new(2, 2).unwrap();
        for y in 0..2u32 {
            for x in 0..2u32 {
                let color = if (x + y) % 2 == 0 {
                    tiny_skia::Color::from_rgba8(255, 0, 0, 255)
                } else {
                    tiny_skia::Color::from_rgba8(0, 0, 255, 255)
                };
                let premul = color.premultiply().to_color_u8();
                pixmap.pixels_mut()[(y * 2 + x) as usize] =
                    tiny_skia::PremultipliedColorU8::from_rgba(
                        premul.red(),
                        premul.green(),
                        premul.blue(),
                        premul.alpha(),
                    )
                    .expect("opaque sample pixel is valid");
            }
        }
        pixmap.encode_png().expect("encode sample png")
    }

    #[test]
    fn png_decodes_to_rgba_dimensions() {
        let img = ImageBits::from_png(&sample_png()).expect("valid png must decode");
        assert_eq!((img.width, img.height), (2, 2));
        assert_eq!(img.data.len(), 2 * 2 * 4);
        // 预乘后 (0,0) 应仍近似纯红（alpha=255 预乘不变）。
        assert_eq!(&img.data[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn invalid_bytes_return_none() {
        assert!(ImageBits::from_png(b"not a png").is_none());
        assert!(ImageBits::from_png(&[]).is_none());
        // PNG 魔数 + 截断载荷。
        let mut truncated = sample_png();
        truncated.truncate(20);
        assert!(ImageBits::from_png(&truncated).is_none());
    }

    /// 构造"合法签名 + IHDR 声明指定尺寸 + 无后续数据"的字节串（解码炸弹
    /// 的最小形态：压缩体积极小，声明尺寸极大）。
    fn header_only_png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&13u32.to_be_bytes()); // IHDR length
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA, deflate, adaptive, no interlace
        bytes
    }

    #[test]
    fn png_decode_bomb_dimensions_rejected_before_allocation() {
        // 审计 C-1：60000×60000（≈14.4 GB 解码分配）必须在 IHDR 阶段拒绝，
        // 不进入 tiny-skia 的分配路径。
        assert!(ImageBits::from_png(&header_only_png(60000, 60000)).is_none());
        // 单边超界（宽 20000 > 16384）。
        assert!(ImageBits::from_png(&header_only_png(20000, 100)).is_none());
        // 总像素超界（16384×8192 = 2^27 > 2^26）。
        assert!(ImageBits::from_png(&header_only_png(16384, 8192)).is_none());
    }

    #[test]
    fn png_ihdr_limits_accept_boundary_and_reject_zero() {
        assert!(dimensions_within_limits(16384, 4096)); // 2^26 恰好在上限
        assert!(!dimensions_within_limits(16384, 4097));
        assert!(!dimensions_within_limits(0, 10));
        assert!(!dimensions_within_limits(10, 0));
        // 尺寸在界内但数据截断 → 仍由 decode_png 报"不可用"，语义一致。
        assert!(ImageBits::from_png(&header_only_png(4, 4)).is_none());
    }
}
