//! W-1 窗口 demo：HTML + CSS → 渲染 → winit 真窗口显示。
//!
//! 经 shell 的 [`App::run`] 便捷入口启动——winit 事件循环、窗口创建、
//! softbuffer 表面全部封装在 crate 内部，公共 API 不泄漏 winit/softbuffer
//! 类型（对齐 decoupling ADR）。窗口可缩放（尺寸变化重渲染）、可关闭。
//! 直接构造 [`PlatformWindow`] 的演示由 W-4 的 `HeadlessWindow`（可无参
//! 构造后端）承担。
//!
//! 运行：
//! ```text
//! cargo run -p muskitty-shell --example window_demo
//! ```

use muskitty_shell::app::App;

const HTML: &str = r#"
<!doctype html>
<html>
  <body>
    <div style="background-color: #2196f3; width: 600px; height: 300px; border-width: 4px; border-style: solid; border-color: #0d47a1">
      <div style="background-color: #ffeb3b; width: 200px; height: 120px; border-width: 2px; border-style: solid; border-color: #f57f17"></div>
    </div>
    <p style="font-size: 32px; color: #212121">Hello MusKitty</p>
    <p style="font-size: 20px; color: #757575">DOM → CSS → Layout → Render</p>
  </body>
</html>
"#;

const CSS: &str = r#"
div { display: block; }
body { margin: 0; }
"#;

fn main() {
    App::run(HTML, CSS);
}
