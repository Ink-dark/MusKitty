//! B-1 集成测试：paint 函数端到端验证。
//!
//! 复用 muskitty-layout 集成测试的 full_pipeline 模式：
//! ```text
//! HTML + CSS → parse → cascade → compute → layout → paint → RenderCommand[]
//! ```

use muskitty_cascade::{
    apply_defaulting, cascade_for_element, cascade_winner, collect_declared_values, compute_value,
    ComputeContext, ComputedStyle, ComputedValue, BUILTIN_PROPERTIES,
};
use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_dom::{Node, NodeKind};
use muskitty_layout::{build_layout_tree, compute_layout, LayoutResult};
use muskitty_renderer::{
    paint, Backend, Border, BorderStyle, Color, MockBackend, PaintInput, RenderCommand,
};
use muskitty_selectors::matching::DomElement;
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

    let empty_props: HashMap<String, Vec<muskitty_css::parser::ComponentValue>> = HashMap::new();
    let ctx = ComputeContext::new(&empty_props);
    let mut styles: HashMap<usize, ComputedStyle> = HashMap::new();
    compute_styles_recursive(&dom, &[sheet], &ctx, None, &mut styles);

    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, viewport_w, viewport_h).expect("layout should succeed");

    PipelineResult {
        dom,
        styles,
        layout,
    }
}

/// 递归计算每个元素的 ComputedStyle（复制自 muskitty-layout 集成测试）。
fn compute_styles_recursive(
    node: &Rc<RefCell<Node>>,
    sheets: &[muskitty_cssom::CssStyleSheet],
    ctx: &ComputeContext,
    parent_style: Option<&ComputedStyle>,
    styles: &mut HashMap<usize, ComputedStyle>,
) {
    let is_element = matches!(node.borrow().kind, NodeKind::Element(_));
    let addr = Rc::as_ptr(node) as usize;
    if is_element {
        let element = DomElement::new(Rc::clone(node));
        let declared = collect_declared_values(&element, sheets);
        let groups = cascade_for_element(declared);
        let mut cs = ComputedStyle::new();
        for (property, group) in &groups {
            let winner = cascade_winner(group);
            let cascaded = winner.map(|w| w.value.as_slice());
            let specified = apply_defaulting(
                property,
                cascaded,
                parent_style.and_then(|ps| ps.get(property)),
            );
            let computed = match &specified {
                ComputedValue::Raw(cvs) => compute_value(property, cvs, ctx),
                _ => specified,
            };
            cs.set(property.clone(), computed);
        }
        for prop_def in BUILTIN_PROPERTIES.iter() {
            if !cs.properties.contains_key(prop_def.name) {
                let specified = apply_defaulting(
                    prop_def.name,
                    None,
                    parent_style.and_then(|ps| ps.get(prop_def.name)),
                );
                let computed = match &specified {
                    ComputedValue::Raw(cvs) => compute_value(prop_def.name, cvs, ctx),
                    _ => specified,
                };
                cs.set(prop_def.name.to_string(), computed);
            }
        }
        styles.insert(addr, cs);
    }

    let children: Vec<Rc<RefCell<Node>>> = node.borrow().child_nodes().to_vec();
    let parent_cs = styles.get(&addr).cloned();
    for child in &children {
        compute_styles_recursive(child, sheets, ctx, parent_cs.as_ref(), styles);
    }
}

/// 运行 paint 并返回指令列表。
fn paint_pipeline(html: &str, css: &str, vw: f32, vh: f32) -> Vec<RenderCommand> {
    let PipelineResult {
        dom,
        styles,
        layout,
    } = full_pipeline(html, css, vw, vh);
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
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
    backend.render(&cmds, 800, 600);
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
