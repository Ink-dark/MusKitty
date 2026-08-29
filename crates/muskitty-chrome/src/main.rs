//! MusKitty 浏览器演示入口：chrome（标签栏 + 工具栏 + 地址栏）+ 页面。
//!
//! 运行：`cargo run -p muskitty-chrome`

use muskitty_chrome::app::App;

const DEMO_HTML: &str = r#"
<!doctype html>
<html>
  <body>
    <div style="background-color: #2196f3; width: 600px; height: 300px; border-width: 4px; border-style: solid; border-color: #0d47a1">
      <div style="background-color: #ffeb3b; width: 200px; height: 120px; border-width: 2px; border-style: solid; border-color: #f57f17"></div>
    </div>
    <p style="font-size: 28px; color: #212121">Hello MusKitty Chrome</p>
  </body>
</html>
"#;

const DEMO_CSS: &str = r#"
div { display: block; }
body { margin: 0; }
"#;

fn main() {
    App::run(DEMO_HTML, DEMO_CSS);
}
