//! chrome 自绘三件套：布局（model）→ 绘制（paint）→ 命中测试（input），
//! 全部纯函数、零外部依赖类型（tiny-skia/cosmic-text 仅在 paint 内部使用）。

pub mod model;
