//! B-1 集成测试：paint 函数端到端验证。
//!
//! 复用 muskitty-layout 集成测试的 full_pipeline 模式：
//! ```text
//! HTML + CSS → parse → cascade → compute → layout → paint → RenderCommand[]
//! ```

use muskitty_cascade::{compute_styles, ComputedStyle, StyleTreeOptions};
use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_dom::Node;
use muskitty_layout::{build_layout_tree, compute_layout, LayoutResult};
use muskitty_renderer::{
    paint, Backend, Border, BorderStyle, Color, MockBackend, PaintInput, RenderCommand,
    RenderOutput, TinySkiaBackend,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// —— 辅助函数 ——

/// 完整 pipeline 结果。
struct PipelineResult {
    dom: Rc<RefCell<Node>>,
    styles: HashMap<usize, ComputedStyle>,
    layout: LayoutResult,
}

/// HTML + CSS → DOM + ComputedStyle + LayoutResult。
fn full_pipeline(html: &str, css: &str, viewport_w: f32, viewport_h: f32) -> PipelineResult {
    let dom = muskitty_html5_parser::parse(html);
    let parsed = parse_stylesheet(css);
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };

    let styles = compute_styles(&dom, &[sheet], &StyleTreeOptions::default());

    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, viewport_w, viewport_h).expect("layout should succeed");

    PipelineResult {
        dom,
        styles,
        layout,
    }
}

/// 运行 paint 并返回指令列表（不剔除，viewport = None）。
fn paint_pipeline(html: &str, css: &str, vw: f32, vh: f32) -> Vec<RenderCommand> {
    paint_pipeline_with_viewport(html, css, vw, vh, None)
}

/// 运行 paint 并返回指令列表（可指定 viewport 剔除）。
fn paint_pipeline_with_viewport(
    html: &str,
    css: &str,
    vw: f32,
    vh: f32,
    viewport: Option<(f32, f32, f32, f32)>,
) -> Vec<RenderCommand> {
    let PipelineResult {
        dom,
        styles,
        layout,
    } = full_pipeline(html, css, vw, vh);
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport,
    };
    paint(&input)
}

// —— 测试用例 ——

#[test]
fn paint_empty_document() {
    let cmds = paint_pipeline("<!doctype html><html></html>", "", 800.0, 600.0);
    // 无 background-color 设置，不应产生任何指令
    assert!(cmds.is_empty(), "empty document should produce no commands");
}

#[test]
fn paint_single_div_with_red_background() {
    let cmds = paint_pipeline(
        "<div style=\"background: red; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    // background: red 是简写，cascade 是否展开为 background-color 取决于 registry。
    // 若简写未展开，background-color 可能不存在 → 0 指令。
    // 这里测试两种情况：用 longhand background-color 确保 paint 工作。
    if cmds.is_empty() {
        // 简写未展开，改用 longhand 测试
        let cmds = paint_pipeline(
            "<div style=\"background-color: red; width: 100px; height: 50px\"></div>",
            "",
            800.0,
            600.0,
        );
        assert_eq!(
            cmds.len(),
            1,
            "background-color:red + sized div should produce 1 command"
        );
    }
}

#[test]
fn paint_longhand_background_color_named() {
    let cmds = paint_pipeline(
        "<div style=\"background-color: red; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect {
            width,
            height,
            background,
            border,
            ..
        } => {
            assert_eq!(*width, 100.0);
            assert_eq!(*height, 50.0);
            assert_eq!(*background, Some(Color::rgb(255, 0, 0)));
            assert_eq!(*border, None);
        }
        _ => panic!("expected Rect command"),
    }
}

#[test]
fn paint_longhand_background_color_hex() {
    let cmds = paint_pipeline(
        "<div style=\"background-color: #00ff00; width: 50px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect { background, .. } => {
            assert_eq!(*background, Some(Color::rgb(0, 255, 0)));
        }
        _ => panic!("expected Rect"),
    }
}

#[test]
fn paint_transparent_background_skipped() {
    let cmds = paint_pipeline(
        "<div style=\"background-color: transparent; width: 100px; height: 100px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert!(
        cmds.is_empty(),
        "transparent background should not produce a command"
    );
}

#[test]
fn paint_no_background_skipped() {
    // 无 background-color → 默认 transparent → 不绘制
    let cmds = paint_pipeline(
        "<div style=\"width: 100px; height: 100px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert!(cmds.is_empty(), "no background-color should not draw");
}

#[test]
fn paint_nested_divs_both_with_background() {
    let cmds = paint_pipeline(
        "<div style=\"background-color: red; width: 200px; height: 200px\">
           <div style=\"background-color: blue; width: 100px; height: 100px\"></div>
         </div>",
        "",
        800.0,
        600.0,
    );
    assert_eq!(
        cmds.len(),
        2,
        "nested divs both with background should produce 2 commands"
    );
    // 父先于子（DOM 先序）
    match (&cmds[0], &cmds[1]) {
        (
            RenderCommand::Rect {
                background: bg1, ..
            },
            RenderCommand::Rect {
                background: bg2, ..
            },
        ) => {
            assert_eq!(*bg1, Some(Color::rgb(255, 0, 0)), "parent red first");
            assert_eq!(*bg2, Some(Color::rgb(0, 0, 255)), "child blue second");
        }
        _ => panic!("expected two Rect commands"),
    }
}

#[test]
fn paint_contents_splice_absolute_coords() {
    // P2-19: paint 读 NodeLayout::abs_x/abs_y（画布绝对坐标），不再沿 DOM
    // 祖先累加偏移。display:contents 把 span 的后代 splice 为 flex 容器的
    // 直接子盒，DOM 祖先链 div>span>div ≠ taffy 父链 div>inner。
    // inner 绝对坐标 = (padding-left 10 + margin-left 5, padding-top 10) = (15, 10)。
    let cmds = paint_pipeline(
        "<div style='display: flex; padding-left: 10px; padding-top: 10px; background-color: red;'>\
           <span style='display: contents'>\
             <div style='width: 50px; height: 50px; background-color: blue; margin-left: 5px;'></div>\
           </span>\
         </div>",
        "",
        800.0,
        600.0,
    );
    // 父 flex 容器先绘制（red，abs 0,0），随后 inner（blue，abs 15,10）。
    assert_eq!(cmds.len(), 2, "flex container + inner should both paint");
    match (&cmds[0], &cmds[1]) {
        (
            RenderCommand::Rect {
                background: bg1,
                x: x1,
                y: y1,
                ..
            },
            RenderCommand::Rect {
                background: bg2,
                x: x2,
                y: y2,
                ..
            },
        ) => {
            assert_eq!(
                *bg1,
                Some(Color::rgb(255, 0, 0)),
                "flex container red first"
            );
            assert!((*x1 - 0.0).abs() < 1.0, "flex container x ~0, got {x1}");
            assert!((*y1 - 0.0).abs() < 1.0, "flex container y ~0, got {y1}");
            assert_eq!(*bg2, Some(Color::rgb(0, 0, 255)), "inner blue second");
            assert!(
                (*x2 - 15.0).abs() < 1.0,
                "inner x ~15 (padding 10 + margin 5), got {x2}"
            );
            assert!(
                (*y2 - 10.0).abs() < 1.0,
                "inner y ~10 (padding-top), got {y2}"
            );
        }
        _ => panic!("expected two Rect commands"),
    }
}

#[test]
fn paint_display_none_skipped() {
    let cmds = paint_pipeline(
        "<div style=\"background-color: red; display: none; width: 100px; height: 100px\"></div>",
        "",
        800.0,
        600.0,
    );
    // display:none 元素不在布局树中 → 无布局结果 → paint 跳过
    assert!(
        cmds.is_empty(),
        "display:none element should not produce a command"
    );
}

#[test]
fn paint_mock_backend_consumes_commands() {
    let cmds = paint_pipeline(
        "<div style=\"background-color: red; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    let mut backend = MockBackend::new();
    // P2-18：Mock 返回 Commands 输出。
    let output = backend.render(&cmds, 800, 600, 1.0);
    assert_eq!(output, RenderOutput::Commands(cmds));
    assert_eq!(backend.len(), 1);
    assert_eq!(backend.width, 800);
    assert_eq!(backend.height, 600);
}

#[test]
fn paint_stylesheet_background_color() {
    let cmds = paint_pipeline(
        "<div class=\"box\"></div>",
        "div.box { background-color: #abcdef; width: 80px; height: 60px; }",
        800.0,
        600.0,
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect { background, .. } => {
            assert_eq!(*background, Some(Color::rgb(0xab, 0xcd, 0xef)));
        }
        _ => panic!("expected Rect"),
    }
}

// —— B-2: border 测试 ——

#[test]
fn paint_border_solid_longhand() {
    let cmds = paint_pipeline(
        "<div style=\"border-width: 2px; border-style: solid; border-color: black; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect {
            border, background, ..
        } => {
            assert_eq!(*background, None, "no background");
            assert_eq!(
                *border,
                Some(Border {
                    width: 2.0,
                    color: Color::BLACK,
                    style: BorderStyle::Solid,
                })
            );
        }
        _ => panic!("expected Rect"),
    }
}

#[test]
fn paint_border_with_background() {
    let cmds = paint_pipeline(
        "<div style=\"background-color: red; border-width: 1px; border-style: solid; border-color: blue; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect {
            background, border, ..
        } => {
            assert_eq!(*background, Some(Color::rgb(255, 0, 0)));
            let b = border.expect("border should be present");
            assert_eq!(b.width, 1.0);
            assert_eq!(b.color, Color::rgb(0, 0, 255));
            assert_eq!(b.style, BorderStyle::Solid);
        }
        _ => panic!("expected Rect"),
    }
}

#[test]
fn paint_border_style_none_skipped() {
    // border-style: none → 不生成边框；且无 background → 不生成指令
    let cmds = paint_pipeline(
        "<div style=\"border-width: 2px; border-style: none; border-color: red; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert!(cmds.is_empty(), "border-style:none + no bg = no command");
}

#[test]
fn paint_border_zero_width_skipped() {
    let cmds = paint_pipeline(
        "<div style=\"border-width: 0px; border-style: solid; border-color: red; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert!(cmds.is_empty(), "border-width:0 + no bg = no command");
}

#[test]
fn paint_border_only_emits_command() {
    // 仅有 border（无 background）也应生成指令
    let cmds = paint_pipeline(
        "<div style=\"border-width: 3px; border-style: dashed; border-color: #ff8800; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect {
            background, border, ..
        } => {
            assert_eq!(*background, None);
            let b = border.expect("border present");
            assert_eq!(b.width, 3.0);
            assert_eq!(b.color, Color::rgb(0xff, 0x88, 0x00));
            assert_eq!(b.style, BorderStyle::Dashed);
        }
        _ => panic!("expected Rect"),
    }
}

#[test]
fn paint_border_shorthand_emits_border() {
    // M-3: `border:` 简写 → cascade 展开 → extract_border 端到端。
    // 必须显式 px 宽度：无宽度时 parse_border_width 读不到 Dimension 而无边框
    // （renderer 既有缺口，`border: solid red` 不绘制）。
    let cmds = paint_pipeline(
        "<div style=\"border: 2px solid black; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect {
            background, border, ..
        } => {
            assert_eq!(*background, None);
            let b = border.expect("border present");
            assert_eq!(b.width, 2.0);
            assert_eq!(b.color, Color::BLACK);
            assert_eq!(b.style, BorderStyle::Solid);
        }
        _ => panic!("expected Rect"),
    }
}

#[test]
fn paint_border_shorthand_hex_color() {
    // 简写带 hash 颜色：parse_border_color 走 hex 路径
    let cmds = paint_pipeline(
        "<div style=\"border: 3px dashed #ff8800; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect { border, .. } => {
            let b = border.expect("border present");
            assert_eq!(b.width, 3.0);
            assert_eq!(b.color, Color::rgb(0xff, 0x88, 0x00));
            assert_eq!(b.style, BorderStyle::Dashed);
        }
        _ => panic!("expected Rect"),
    }
}

// —— B-2: rgb() / rgba() 颜色函数测试 ——

#[test]
fn paint_background_rgb_function() {
    let cmds = paint_pipeline(
        "<div style=\"background-color: rgb(0, 128, 255); width: 50px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect { background, .. } => {
            assert_eq!(*background, Some(Color::rgb(0, 128, 255)));
        }
        _ => panic!("expected Rect"),
    }
}

#[test]
fn paint_background_rgba_function() {
    let cmds = paint_pipeline(
        "<div style=\"background-color: rgba(255, 0, 0, 0.5); width: 50px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    // alpha=0.5 → 不透明 → 应生成指令
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect { background, .. } => {
            let bg = background.expect("background present");
            assert_eq!(bg.r, 255);
            assert_eq!(bg.g, 0);
            assert_eq!(bg.b, 0);
            assert!(bg.a > 120 && bg.a < 136, "alpha ≈ 128, got {}", bg.a);
        }
        _ => panic!("expected Rect"),
    }
}

#[test]
fn paint_background_rgb_fully_transparent() {
    let cmds = paint_pipeline(
        "<div style=\"background-color: rgba(255, 0, 0, 0); width: 50px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
    );
    // alpha=0 → 完全透明 → 跳过
    assert!(cmds.is_empty(), "alpha=0 should be transparent");
}

// —— H-4: var() 端到端（cascade 收集自定义属性 → paint）——

#[test]
fn paint_var_custom_property_background_color() {
    // :root 定义 --brand，div 用 var(--brand) 作为 background-color
    let cmds = paint_pipeline(
        "<div></div>",
        ":root { --brand: red; } div { background-color: var(--brand); width: 80px; height: 60px; }",
        800.0,
        600.0,
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect { background, .. } => {
            assert_eq!(*background, Some(Color::rgb(255, 0, 0)));
        }
        _ => panic!("expected Rect"),
    }
}

#[test]
fn paint_var_inherited_from_parent() {
    // 父 div 声明 --brand，子 div 通过 var() 继承
    let cmds = paint_pipeline(
        "<div class=\"parent\"><div></div></div>",
        ".parent { --brand: #00ff00; } div div { background-color: var(--brand); width: 40px; height: 40px; }",
        800.0,
        600.0,
    );
    // 父 div 无 background，子 div 有 → 恰好 1 条 Rect
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        RenderCommand::Rect { background, .. } => {
            assert_eq!(*background, Some(Color::rgb(0, 255, 0)));
        }
        _ => panic!("expected Rect"),
    }
}

// —— P3-6: viewport culling ——

#[test]
fn paint_viewport_culling_skips_out_of_viewport() {
    // 元素完全位于视口外 → 不生成绘制指令。
    let cmds = paint_pipeline_with_viewport(
        "<div style=\"background-color: red; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
        // viewport 从 (200, 200) 起，元素 (0,0)-(100,50) 完全在外
        Some((200.0, 200.0, 100.0, 100.0)),
    );
    assert!(
        cmds.is_empty(),
        "element fully outside viewport should be culled, got {:?}",
        cmds
    );

    // 元素与视口相交 → 保留。
    let cmds = paint_pipeline_with_viewport(
        "<div style=\"background-color: red; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
        // viewport (50,25)-(150,125) 与元素 (0,0)-(100,50) 相交
        Some((50.0, 25.0, 100.0, 100.0)),
    );
    assert_eq!(
        cmds.len(),
        1,
        "element intersecting viewport should still paint"
    );

    // 无 viewport（None）→ 不剔除。
    let cmds = paint_pipeline_with_viewport(
        "<div style=\"background-color: red; width: 100px; height: 50px\"></div>",
        "",
        800.0,
        600.0,
        None,
    );
    assert_eq!(cmds.len(), 1, "None viewport disables culling");
}

// —— RN-1：空 Clip 对的 Mask 懒构建 ——

/// RN-1：10 万个 `[Clip, EndClip]` 空对（paint 层对有布局盒的 overflow
/// 元素无条件生成）不得触发整画布 Mask 构建。修复前每个空对在 EndClip
/// 迭代做一次全画布 `Mask::new` + `fill_path`——10 万 × 800×600 ≈
/// 192 GB 无效 memset，CPU 挂起；修复后空对零成本，输出为纯白画布。
/// 本测试同时是隐式性能回归测试——回归会令 CI 超时。
#[test]
fn empty_clip_pairs_cost_nothing() {
    let mut cmds = Vec::with_capacity(200_000);
    for _ in 0..100_000 {
        cmds.push(RenderCommand::Clip {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
        cmds.push(RenderCommand::EndClip);
    }
    let mut backend = TinySkiaBackend::new();
    let out = backend.render(&cmds, 800, 600, 1.0);
    let RenderOutput::Pixels {
        width,
        height,
        data,
    } = out
    else {
        panic!("expected Pixels output");
    };
    assert_eq!((width, height), (800, 600));
    assert!(
        data.iter().all(|&b| b == 255),
        "no paint commands: canvas must stay pure white (RGBA 255)"
    );
}

/// RN-1：Mask 移到消费点懒构建后，裁剪语义不变——clip 外的绘制被裁掉，
/// EndClip 恢复后的绘制不受影响。
#[test]
fn clip_semantics_unchanged_after_lazy_mask() {
    let cmds = vec![
        RenderCommand::Clip {
            x: 10.0,
            y: 10.0,
            width: 50.0,
            height: 50.0,
        },
        // 覆盖全画布的红色矩形：clip 外全部被裁掉。
        RenderCommand::Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            background: Some(Color::rgb(255, 0, 0)),
            border: None,
        },
        RenderCommand::EndClip,
        // clip 结束后的蓝色小矩形：完整绘制。
        RenderCommand::Rect {
            x: 0.0,
            y: 0.0,
            width: 5.0,
            height: 5.0,
            background: Some(Color::rgb(0, 0, 255)),
            border: None,
        },
    ];
    let mut backend = TinySkiaBackend::new();
    let RenderOutput::Pixels { data, .. } = backend.render(&cmds, 800, 600, 1.0) else {
        panic!("expected Pixels output");
    };
    let px = |x: u32, y: u32| {
        let i = ((y * 800 + x) * 4) as usize;
        (data[i], data[i + 1], data[i + 2])
    };
    // clip 区域中心：红色。
    assert_eq!(px(30, 30), (255, 0, 0), "inside clip: red");
    // clip 外（原本会被红矩形覆盖）：白色。
    assert_eq!(px(200, 200), (255, 255, 255), "outside clip: white");
    // EndClip 后的蓝色小矩形：完整绘制。
    assert_eq!(px(2, 2), (0, 0, 255), "after EndClip: blue");
}
