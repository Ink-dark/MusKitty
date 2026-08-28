//! MusKitty 浏览器外壳二进制入口。
//!
//! 启动默认示例页面的真窗口（HTML+CSS → 渲染 → softbuffer 显示）。
//!
//! 运行：
//! ```text
//! cargo run -p muskitty-shell
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
