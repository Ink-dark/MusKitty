//! B-4 端到端 demo：HTML + CSS → cascade → layout → paint → PNG。
//!
//! 运行：
//! ```text
//! cargo run --example render_demo
//! ```
//!
//! 输出 `render_demo.png` 到当前工作目录。

use muskitty_cascade::{
    apply_defaulting, cascade_for_element, cascade_winner, collect_custom_properties,
    collect_declared_values, compute_value, ComputeContext, ComputedStyle, ComputedValue,
    BUILTIN_PROPERTIES,
};
use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_dom::{Node, NodeKind};
use muskitty_layout::{build_layout_tree, compute_layout};
use muskitty_renderer::{paint, Backend, PaintInput, TinySkiaBackend};
use muskitty_selectors::matching::DomElement;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const VIEWPORT_W: f32 = 800.0;
const VIEWPORT_H: f32 = 600.0;

const HTML: &str = r#"
<!doctype html>
<html>
  <body>
    <div style="background-color: #2196f3; width: 600px; height: 400px; border-width: 4px; border-style: solid; border-color: #0d47a1">
      <div style="background-color: #ffeb3b; width: 200px; height: 200px; border-width: 2px; border-style: solid; border-color: #f57f17"></div>
      <div style="background-color: #f44336; width: 200px; height: 150px; border-width: 2px; border-style: solid; border-color: #b71c1c"></div>
    </div>
  </body>
</html>
"#;

const CSS: &str = r#"
div { display: block; }
body { margin: 0; }
"#;

fn main() {
    // 1. 解析 HTML → DOM
    let dom = muskitty_html5_parser::parse(HTML);

    // 2. 解析 CSS → CssStyleSheet
    let parsed = parse_stylesheet(CSS);
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };

    // 3. cascade + compute → 每元素 ComputedStyle
    let mut styles: HashMap<usize, ComputedStyle> = HashMap::new();
    compute_styles_recursive(&dom, &[sheet], None, None, &mut styles);

    // 4. layout → LayoutResult
    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, VIEWPORT_W, VIEWPORT_H).expect("layout failed");

    // 5. paint → RenderCommand[]
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
    };
    let commands = paint(&input);

    println!("paint produced {} render commands", commands.len());
    for (i, cmd) in commands.iter().enumerate() {
        println!("  [{}] {:?}", i, cmd);
    }

    // 6. render → PNG via tiny-skia
    let mut backend = TinySkiaBackend::new();
    backend.render(&commands, VIEWPORT_W as u32, VIEWPORT_H as u32);

    let out_path = "render_demo.png";
    backend
        .save_png(out_path)
        .unwrap_or_else(|e| panic!("failed to save PNG to {}: {}", out_path, e));

    println!();
    println!("✓ Rendered to {}", out_path);
    println!("  Viewport: {}x{}", VIEWPORT_W, VIEWPORT_H);
    println!("  Commands: {}", commands.len());
}

/// 递归计算每个元素的 ComputedStyle。
fn compute_styles_recursive(
    node: &Rc<RefCell<Node>>,
    sheets: &[muskitty_cssom::CssStyleSheet],
    parent_props: Option<&HashMap<String, Vec<muskitty_css::parser::ComponentValue>>>,
    parent_style: Option<&ComputedStyle>,
    styles: &mut HashMap<usize, ComputedStyle>,
) {
    let is_element = matches!(node.borrow().kind, NodeKind::Element(_));
    let addr = Rc::as_ptr(node) as usize;
    let empty_props: HashMap<String, Vec<muskitty_css::parser::ComponentValue>> = HashMap::new();
    let parent_props = parent_props.unwrap_or(&empty_props);
    let mut props: HashMap<String, Vec<muskitty_css::parser::ComponentValue>> = HashMap::new();
    if is_element {
        let element = DomElement::new(Rc::clone(node));
        props = collect_custom_properties(&element, sheets, parent_props);
        let ctx = ComputeContext::new(&props);
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
                ComputedValue::Raw(cvs) => compute_value(property, cvs, &ctx),
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
                    ComputedValue::Raw(cvs) => compute_value(prop_def.name, cvs, &ctx),
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
        compute_styles_recursive(child, sheets, Some(&props), parent_cs.as_ref(), styles);
    }
}
