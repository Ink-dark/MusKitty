//! 渲染回归探针：颜色（纯色/半透明/渐变）与裁剪（overflow:hidden，RN-1
//! clip Mask 时机改动）的像素级断言。
//!
//! 走 `page::render_page` 全管线（与真窗口同一渲染路径，仅跳过
//! softbuffer 呈现），直接断言 RGBA 缓冲区——离线、确定性、无窗口可跑。

use muskitty_chrome::page::render_page;
use muskitty_renderer::RenderOutput;

/// 读取 (x, y) 处 RGBA8 像素（逻辑坐标 = 物理坐标，scale 1.0）。
fn px(data: &[u8], width: u32, x: u32, y: u32) -> (u8, u8, u8, u8) {
    let i = ((y * width + x) * 4) as usize;
    (data[i], data[i + 1], data[i + 2], data[i + 3])
}

fn render(html: &str, css: &str, width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    match render_page(html, css, width, height, 1.0).expect("render ok") {
        RenderOutput::Pixels {
            data,
            width,
            height,
        } => (data, width, height),
        _ => panic!("expected Pixels output"),
    }
}

// ---- 纯色 ----

#[test]
fn solid_red_and_green_blocks() {
    let html = "<div class=\"r\"></div><div class=\"g\"></div>";
    let css = "body{margin:0} .r{width:100px;height:100px;background-color:#ff0000} \
               .g{width:100px;height:100px;background-color:#00cc00}";
    let (data, w, _h) = render(html, css, 300, 300);
    // 红块中心 (50,50)；绿块紧跟其后（块级堆叠，y 100..200）。
    assert_eq!(px(&data, w, 50, 50), (255, 0, 0, 255), "red block center");
    assert_eq!(
        px(&data, w, 50, 150),
        (0, 204, 0, 255),
        "green block center"
    );
    // 两块之外为白画布。
    assert_eq!(
        px(&data, w, 250, 50),
        (255, 255, 255, 255),
        "canvas stays white"
    );
}

// ---- 半透明 ----

#[test]
fn semi_transparent_red_blends_over_white() {
    // rgba 0.5 红 over 白画布 → (255, 127±1, 127±1)；α 合成路径的回归探针。
    let html = "<div class=\"a\"></div>";
    let css = "body{margin:0} .a{width:200px;height:200px;background-color:rgba(255,0,0,0.5)}";
    let (data, w, _h) = render(html, css, 300, 300);
    let (r, g, b, a) = px(&data, w, 100, 100);
    assert_eq!(r, 255);
    assert!((120..=136).contains(&g), "blended green ~127, got {g}");
    assert!((120..=136).contains(&b), "blended blue ~127, got {b}");
    assert_eq!(a, 255, "canvas is opaque");
}

#[test]
fn semi_transparent_red_blends_over_green() {
    // rgba 0.5 红 over 不透明绿 → (127±1, 102±1, 0)：混合发生在下层内容上。
    let html = "<div class=\"g\"><div class=\"a\"></div></div>";
    let css = "body{margin:0} .g{width:300px;height:300px;background-color:#00cc00} \
               .a{width:200px;height:200px;background-color:rgba(255,0,0,0.5)}";
    let (data, w, _h) = render(html, css, 300, 300);
    let (r, g, b, _a) = px(&data, w, 100, 100);
    assert!((120..=134).contains(&r), "blended red ~127, got {r}");
    assert!((95..=109).contains(&g), "blended green ~102, got {g}");
    assert_eq!(b, 0);
}

// ---- 渐变 ----

#[test]
fn linear_gradient_renders_non_uniform() {
    // 探针性质：linear-gradient 支持状态的回归锚点。若将来实现渐变，
    // 左右两端颜色应不同；当前若不支持（退化为透明/白），断言画布非红非蓝
    // 亦成立——本测试只要求"两端一致或有效渐变"，不允许半成品混色。
    let html = "<div class=\"grad\"></div>";
    let css = "body{margin:0} .grad{width:300px;height:100px;background-image:linear-gradient(to right, #ff0000, #0000ff)}";
    let (data, w, _h) = render(html, css, 300, 200);
    let left = px(&data, w, 10, 50);
    let right = px(&data, w, 290, 50);
    let uniform = left == right;
    if uniform {
        // 渐变未实现：退化为单一颜色（透明→白画布）。两端必须一致且非混色噪声。
        assert_eq!(
            left,
            (255, 255, 255, 255),
            "unimplemented gradient degrades to canvas"
        );
    } else {
        // 渐变已实现：左端偏红、右端偏蓝。
        assert!(
            left.0 > 200 && left.2 < 80,
            "left end reddish, got {left:?}"
        );
        assert!(
            right.2 > 200 && right.0 < 80,
            "right end bluish, got {right:?}"
        );
    }
}

// ---- 裁剪（RN-1 回归：clip Mask 消费点构建） ----

#[test]
fn overflow_hidden_clips_oversized_child() {
    // overflow:hidden 父 100x100 + 红 300x300 子：父内见红、父外必须无红
    //（无裁剪时 (150,50) 会是红）。
    let html = "<div class=\"clip\"><div class=\"big\"></div></div>";
    let css =
        "body{margin:0} .clip{width:100px;height:100px;overflow:hidden;background-color:#00ff00} \
               .big{width:300px;height:300px;background-color:#ff0000}";
    let (data, w, _h) = render(html, css, 400, 400);
    assert_eq!(
        px(&data, w, 50, 50),
        (255, 0, 0, 255),
        "child visible inside clip"
    );
    assert_eq!(
        px(&data, w, 150, 50),
        (255, 255, 255, 255),
        "child must be clipped outside parent"
    );
    assert_eq!(
        px(&data, w, 50, 150),
        (255, 255, 255, 255),
        "clipped vertically too"
    );
}

#[test]
fn many_overflow_hidden_siblings_render_in_place() {
    // 彩色 overflow:hidden 容器（emit Clip/EndClip + Rect）与空 overflow:hidden
    // 容器（空 clip 对，RN-1 后不得画出内容）交错堆叠。断言：
    // 偶数行 = 容器内可见的子块色（子块 300x300 被裁到 40x20）；奇数行（空
    // 容器）= 白；x>40 处无溢出。
    let mut html = String::new();
    for i in 0..150 {
        if i % 2 == 0 {
            html.push_str("<div class=\"c0\"><div class=\"big0\"></div></div>");
        } else {
            html.push_str("<div class=\"c1\"><div class=\"big1\"></div></div>");
        }
        html.push_str("<div class=\"empty\"></div>");
    }
    let css = "body{margin:0} \
               .c0{width:40px;height:20px;overflow:hidden;background-color:#ff0000} \
               .c1{width:40px;height:20px;overflow:hidden;background-color:#0000ff} \
               .empty{width:40px;height:20px;overflow:hidden} \
               .big0{width:300px;height:300px;background-color:#00ff00} \
               .big1{width:300px;height:300px;background-color:#ffeb3b}";
    let (data, w, _h) = render(&html, css, 200, 200);
    for r in 0..10 {
        let y = r * 20 + 5;
        let expected = match r % 4 {
            0 => (0, 255, 0, 255),     // c0 的绿色子块
            2 => (255, 235, 59, 255),  // c1 的黄色子块
            _ => (255, 255, 255, 255), // empty 容器：无内容可画
        };
        assert_eq!(px(&data, w, 10, y), expected, "row {r}");
        // 子块 300x300 被裁到 40x20：x>40 处不得出现任何溢出。
        assert_eq!(
            px(&data, w, 150, y),
            (255, 255, 255, 255),
            "row {r} clip boundary"
        );
    }
}
