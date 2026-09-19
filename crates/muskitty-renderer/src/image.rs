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

impl ImageBits {
    /// 解码 PNG 字节为 [`ImageBits`]；非法 PNG / 不支持的格式返回 `None`。
    ///
    /// tiny-skia 的 `decode_png` 已覆盖 8/16-bit、灰度/RGB/索引/带 alpha 全
    /// 变体并统一为 RGBA8；解码失败（含截断、非 PNG 魔数）一律 `None`，
    /// 调用方按"图像不可用 → 跳过背景图"处理（HTML/CSS 语义：资源加载
    /// 失败不阻塞渲染）。
    #[cfg(feature = "backend-tiny-skia")]
    pub fn from_png(bytes: &[u8]) -> Option<Self> {
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
}
