//! chrome 自绘三件套：布局（model）→ 绘制（paint）→ 命中测试（input），
//! 布局/命中为纯函数，绘制仅内部使用 tiny-skia/cosmic-text（不进 pub 签名）。

pub mod model;
pub mod paint;
