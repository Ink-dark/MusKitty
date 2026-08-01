# Phase 3 — Layout 实施计划

> **规范依据**：
> - CSS Display Module Level 3: <https://drafts.csswg.org/css-display-3/>
> - CSS Box Model Module Level 3: <https://drafts.csswg.org/css-box-3/>
> - CSS Flexbox Layout Module Level 1: <https://drafts.csswg.org/css-flexbox-1/>
> - CSS Grid Layout Module Level 2: <https://drafts.csswg.org/css-grid-2/>
> - CSS Writing Modes Level 4: <https://drafts.csswg.org/css-writing-modes-4/>
>
> **依赖 crate**：taffy 0.7.x（Flexbox + Grid + Block layout engine）
>
> **入场门槛**（已满足）：Phase 2 cascade + computed values 完成（71 测试全绿）

## 目标

新建 `muskitty-layout` crate，实现 DOM + ComputedStyle → 布局盒树 → 每元素位置/尺寸的完整 pipeline。

## 架构

```
DOM 树 (Rc<RefCell<Node>>)
  + ComputedStyle per element (from muskitty-cascade)
        │  build_layout_tree
        ▼
LayoutTree (taffy TaffyTree + NodeId 映射)
        │  compute_layout (taffy)
        ▼
LayoutResult (per-element x/y/width/height)
```

单向数据流：DOM + ComputedStyle → LayoutTree → Taffy 计算 → LayoutResult。不反向修改 DOM。

## 批次概览

| 批次 | 内容 | 规范 |
|------|------|------|
| L-0 | Cascade 收尾：inline `style` 属性收集 + 清理 `muskitty-css-values` 死依赖 | §6.1 准则 4 |
| L-1 | crate 骨架 + LayoutTree 类型定义 + taffy 依赖 | 工程基础设施 |
| L-2 | DOM → LayoutTree 转换（build_layout_tree） | CSS Display §2/§3 |
| L-3 | ComputedStyle → taffy::Style 映射（display/box-model/size） | CSS Box Model §2/§3 |
| L-4 | Flexbox 属性映射（flex-direction/justify/align/gap） | CSS Flexbox §4-§8 |
| L-5 | Taffy 布局计算 + LayoutResult 输出提取 | CSS Display §2 |
| L-6 | 端到端集成测试（DOM + CSS → 布局结果验证） | 全链路 |

## L-0: Cascade 收尾

### 目标

在进入 Phase 3 前，补齐 cascade 的两个遗留 gap：
1. inline `style` 属性的 declared value 收集（§6.1 准则 4 当前为死代码）
2. 清理 `muskitty-css-values` 死依赖

### 文件

- 修改: `crates/muskitty-cascade/src/filter.rs`
- 修改: `crates/muskitty-cascade/Cargo.toml`
- 测试: `crates/muskitty-cascade/tests/filter.rs`

### 步骤

- [ ] **步骤 1：写 inline style 收集的失败测试**

在 `tests/filter.rs` 中新增测试 `style_attr_collected`：

```rust
#[test]
fn style_attr_collected() {
    use muskitty_dom::{Node, NodeKind, NodeType, Element, Attribute, Namespace};
    use muskitty_cssom::{CssStyleSheet, CssRule, CssStyleRule, CssDeclaration, Origin};

    // <div style="color: red">
    let mut element = Element::new("div", Namespace::Html);
    element.attributes.push(Attribute {
        name: "style".to_string(),
        value: "color: red".to_string(),
        namespace: None,
        prefix: None,
    });
    let node = Rc::new(RefCell::new(Node::new(NodeKind::Element(element))));
    let dom_element = DomElement::new(Rc::clone(&node));

    let sheets: Vec<CssStyleSheet> = vec![];
    let declared = collect_declared_values(&dom_element, &sheets);

    // 应收集到 1 条来自 style 属性的声明
    let color_decl = declared.iter().find(|d| d.property == "color");
    assert!(color_decl.is_some(), "style attr 'color: red' should be collected");
    let color_decl = color_decl.unwrap();
    assert!(color_decl.from_style_attr, "from_style_attr should be true");
    assert_eq!(color_decl.origin, Origin::Author);
    // style attr 的 specificity 应为 (1,0,0,0) — 最高优先级
}
```

- [ ] **步骤 2：运行测试确认失败**

```powershell
cargo test -p muskitty-cascade --test filter style_attr_collected
```
预期：FAIL（当前 filter.rs 不处理 style 属性）

- [ ] **步骤 3：实现 inline style 收集**

在 `filter.rs` 的 `collect_declared_values` 中，在遍历 stylesheet 之后，添加从 DOM 元素 `style` 属性收集声明的逻辑：

```rust
// 在 collect_declared_values 返回前，收集 inline style 属性
if let Some(style_str) = element.get_attribute("style") {
    // 解析 style 属性值为 declarations
    // 使用 muskitty-css 的 parse_a_declaration 或 parse_a_blocks_contents
    // 对每个 declaration 创建 DeclaredValue {
    //     from_style_attr: true,
    //     specificity: Specificity::style_attr(), // (1,0,0,0)
    //     origin: Origin::Author,
    //     order: *order,
    //     ...
    // }
}
```

需要在 `DomElement` 或 filter 中添加获取 `style` 属性的辅助方法。如果 `DomElement` trait 没有 `get_attribute` 方法，则通过 DOM API 直接访问元素属性。

- [ ] **步骤 4：运行测试确认通过**

```powershell
cargo test -p muskitty-cascade --test filter style_attr_collected
```
预期：PASS

- [ ] **步骤 5：添加 integration 测试验证 style attr 在完整 pipeline 中生效**

在 `tests/integration.rs` 中新增测试 `style_attr_beats_same_specificity`：

```rust
#[test]
fn style_attr_beats_same_specificity() {
    // <p style="color: green"> + CSS p { color: red; }
    // style attr 应赢（§6.1 准则 4）
    // 验证 computed color == green
}
```

- [ ] **步骤 6：清理 muskitty-css-values 死依赖**

从 `crates/muskitty-cascade/Cargo.toml` 的 `[dependencies]` 中移除：
```toml
# 删除这行
muskitty-css-values = { path = "../muskitty-css-values", version = "0.1.0" }
```

- [ ] **步骤 7：运行全部测试 + clippy**

```powershell
cargo test -p muskitty-cascade
cargo clippy -p muskitty-cascade --all-targets -- -D warnings
cargo fmt -p muskitty-cascade -- --check
```

- [ ] **步骤 8：提交**

```bash
git add crates/muskitty-cascade
git commit -m "[cascade] L-0: inline style attr collection + remove dead css-values dependency"
```

## L-1: crate 骨架 + LayoutTree 类型

### 目标

新建 `muskitty-layout` crate，定义布局树核心类型，引入 taffy 依赖。

### 文件

- 创建: `crates/muskitty-layout/Cargo.toml`
- 创建: `crates/muskitty-layout/src/lib.rs`
- 创建: `crates/muskitty-layout/src/tree.rs`
- 修改: `d:\Muskitty\Cargo.toml`（workspace members 加入 layout）
- 修改: `d:\Muskitty\.gitignore`（不需要，layout 在主仓库内）

### 步骤

- [ ] **步骤 1：创建 Cargo.toml**

```toml
[package]
name = "muskitty-layout"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"
license = "Apache-2.0"
description = "CSS Layout engine (box tree construction + taffy layout computation)"
repository = "https://github.com/muskitty-dev/muskitty-layout"
homepage = "https://github.com/muskitty-dev"
documentation = "https://docs.rs/muskitty-layout"
keywords = ["css", "layout", "flexbox", "web"]
categories = ["parser-implementations", "web-programming"]
authors = ["MusCat / MusKitty Bit-Torch Community"]

[dependencies]
muskitty-dom = { path = "../muskitty-dom", version = "0.1.0" }
muskitty-cascade = { path = "../muskitty-cascade", version = "0.1.0" }
muskitty-css = { path = "../muskitty-css", version = "0.5.0" }
muskitty-cssom = { path = "../muskitty-cssom", version = "0.1.0" }
muskitty-selectors = { path = "../muskitty-selectors", version = "0.1.0", features = ["dom"] }
taffy = "0.7"
```

- [ ] **步骤 2：加入 workspace members**

修改主仓库 `Cargo.toml`：
```toml
members = [
    "crates/muskitty-cascade",
    "crates/muskitty-layout",
]
```

- [ ] **步骤 3：创建 lib.rs**

```rust
//! MusKitty Layout — CSS 布局引擎。
//!
//! 将 DOM 树 + ComputedStyle 转换为布局盒树，
//! 使用 taffy 引擎计算 Flexbox/Grid/Block 布局，
//! 输出每个元素的位置和尺寸。
//!
//! # 数据流
//!
//! ```text
//! DOM 树 + ComputedStyle per element
//!     │  build_layout_tree
//!     ▼
//! LayoutTree (taffy TaffyTree + NodeId 映射)
//!     │  compute_layout
//!     ▼
//! LayoutResult (per-element x/y/width/height)
//! ```
//!
//! # 规范依据
//!
//! - CSS Display Level 3: box tree / formatting context
//! - CSS Box Model Level 3: margin/border/padding/content
//! - CSS Flexbox Level 1: flex container/item
//! - CSS Grid Level 2: grid container/item

pub mod tree;
pub mod convert;
pub mod style_map;
pub mod result;

pub use tree::{LayoutTree, LayoutNode};
pub use result::{LayoutResult, NodeLayout};
```

- [ ] **步骤 4：创建 tree.rs — LayoutTree 类型**

```rust
//! 布局树类型定义。
//!
//! LayoutTree 包装 taffy::TaffyTree，维护 DOM Node → taffy NodeId 的映射。

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use muskitty_dom::Node;
use taffy::node::NodeId;
use taffy::TaffyTree;

/// 布局树节点数据。
///
/// 每个 LayoutNode 对应一个 DOM 元素，携带：
/// - DOM 节点引用（用于输出时关联）
/// - taffy NodeId（用于查询布局结果）
#[derive(Debug)]
pub struct LayoutNode {
    /// 对应的 DOM 节点。
    pub dom_node: Rc<RefCell<Node>>,
    /// taffy 中的节点 ID。
    pub taffy_node: NodeId,
}

/// 布局树。
///
/// 包装 taffy::TaffyTree，维护 DOM 节点指针到 taffy NodeId 的映射。
pub struct LayoutTree {
    /// taffy 的内部节点树。
    pub taffy: TaffyTree,
    /// DOM 节点指针地址 → taffy NodeId 映射。
    pub node_map: HashMap<usize, NodeId>,
    /// 根节点 ID。
    pub root: Option<NodeId>,
}

impl LayoutTree {
    /// 创建空布局树。
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            node_map: HashMap::new(),
            root: None,
        }
    }
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **步骤 5：创建占位模块文件**

`src/convert.rs`:
```rust
//! DOM + ComputedStyle → LayoutTree 转换。
```

`src/style_map.rs`:
```rust
//! ComputedStyle → taffy::Style 映射。
```

`src/result.rs`:
```rust
//! 布局计算结果提取。

/// 单个元素的布局结果。
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeLayout {
    /// 相对父元素的位置 X（px）。
    pub x: f32,
    /// 相对父元素的位置 Y（px）。
    pub y: f32,
    /// 计算后的宽度（px）。
    pub width: f32,
    /// 计算后的高度（px）。
    pub height: f32,
}

/// 整棵布局树的结果集合。
#[derive(Debug, Default)]
pub struct LayoutResult {
    /// DOM 节点指针地址 → 布局结果。
    pub nodes: std::collections::HashMap<usize, NodeLayout>,
}

impl LayoutResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, node_addr: usize) -> Option<&NodeLayout> {
        self.nodes.get(&node_addr)
    }
}
```

- [ ] **步骤 6：验证编译**

```powershell
cargo check -p muskitty-layout
```
预期：编译通过（可能有 unused warning，可接受）

- [ ] **步骤 7：提交**

```bash
git add crates/muskitty-layout Cargo.toml
git commit -m "[layout] L-1: crate skeleton + LayoutTree/NodeLayout types"
```

## L-2: DOM → LayoutTree 转换

### 目标

实现 `build_layout_tree`：遍历 DOM 树，为每个元素节点创建 taffy 节点，构建 LayoutTree。

### 文件

- 修改: `crates/muskitty-layout/src/convert.rs`
- 修改: `crates/muskitty-layout/src/lib.rs`（导出 build_layout_tree）
- 创建: `crates/muskitty-layout/tests/build_tree.rs`

### 设计要点

- 只处理 `Element` 节点；`Text` / `Comment` 等非元素节点跳过（或作为 leaf measure 节点，本批次先用 zero-size leaf）
- `display: none` 的元素不创建 taffy 节点
- 构建 taffy 树时需要先创建子节点再设置到父节点（taffy API 要求）
- DOM 节点的 `Rc<RefCell<Node>>` 指针地址用作 HashMap key

### 步骤

- [ ] **步骤 1：写失败测试 — 简单 DOM 树构建**

`tests/build_tree.rs`:
```rust
use muskitty_layout::{LayoutTree, build_layout_tree};
use muskitty_dom::{Node, NodeKind, NodeType, Element, Namespace};
use std::rc::Rc;
use std::cell::RefCell;

fn make_element(tag: &str) -> Rc<RefCell<Node>> {
    Rc::new(RefCell::new(Node::new(NodeKind::Element(
        Element::new(tag, Namespace::Html)
    ))))
}

#[test]
fn single_element_builds_one_node() {
    let root = make_element("div");
    let styles = std::collections::HashMap::new(); // 空ComputedStyle表
    let tree = build_layout_tree(&root, &styles);
    assert!(tree.root.is_some());
    assert_eq!(tree.node_map.len(), 1);
}

#[test]
fn nested_elements_build_tree() {
    // div > p > span
    let root = make_element("div");
    let p = make_element("p");
    let span = make_element("span");
    root.borrow_mut().append_child(Rc::clone(&p));
    p.borrow_mut().append_child(Rc::clone(&span));

    let styles = std::collections::HashMap::new();
    let tree = build_layout_tree(&root, &styles);
    assert_eq!(tree.node_map.len(), 3);
}

#[test]
fn text_nodes_skipped() {
    // div > "hello"
    let root = make_element("div");
    let text = Rc::new(RefCell::new(Node::new(NodeKind::Text("hello".to_string()))));
    root.borrow_mut().append_child(text);

    let styles = std::collections::HashMap::new();
    let tree = build_layout_tree(&root, &styles);
    // 只有 div 一个节点，text 不创建 layout node
    assert_eq!(tree.node_map.len(), 1);
}

#[test]
fn display_none_excluded() {
    // div > p[display:none]
    let root = make_element("div");
    let p = make_element("p");
    root.borrow_mut().append_child(Rc::clone(&p));

    let mut styles = std::collections::HashMap::new();
    let p_addr = Rc::as_ptr(&p) as usize;
    let mut p_style = muskitty_cascade::ComputedStyle::new();
    p_style.set("display", muskitty_cascade::ComputedValue::Keyword("none".to_string()));
    styles.insert(p_addr, p_style);

    let tree = build_layout_tree(&root, &styles);
    // p 被排除，只有 div
    assert_eq!(tree.node_map.len(), 1);
}
```

- [ ] **步骤 2：运行测试确认失败**

```powershell
cargo test -p muskitty-layout --test build_tree
```
预期：FAIL（build_layout_tree 未实现）

- [ ] **步骤 3：实现 build_layout_tree**

在 `convert.rs` 中：

```rust
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use muskitty_dom::{Node, NodeKind};
use muskitty_cascade::ComputedStyle;
use taffy::node::NodeId;
use crate::tree::LayoutTree;
use crate::style_map;

type StyleMap = HashMap<usize, ComputedStyle>;

/// 从 DOM 树 + ComputedStyle 表构建 LayoutTree。
///
/// 递归遍历 DOM：
/// - Element 节点 → 创建 taffy 节点（映射 display/size/margin/padding 到 taffy::Style）
/// - display:none → 跳过
/// - Text/Comment 等非元素 → 跳过（后续批次处理 text measure）
pub fn build_layout_tree(
    root: &Rc<RefCell<Node>>,
    styles: &StyleMap,
) -> LayoutTree {
    let mut tree = LayoutTree::new();
    if let Some(root_id) = build_node_recursive(&mut tree, root, styles) {
        tree.root = Some(root_id);
    }
    tree
}

fn build_node_recursive(
    tree: &mut LayoutTree,
    node: &Rc<RefCell<Node>>,
    styles: &StyleMap,
) -> Option<NodeId> {
    let kind = &node.borrow().kind;
    match kind {
        NodeKind::Element(elem) => {
            let addr = Rc::as_ptr(node) as usize;
            let computed = styles.get(&addr);

            // 检查 display:none
            if let Some(cs) = computed {
                if let Some(muskitty_cascade::ComputedValue::Keyword(kw)) = cs.get("display") {
                    if kw.eq_ignore_ascii_case("none") {
                        return None; // 跳过
                    }
                }
            }

            // 映射 ComputedStyle → taffy::Style
            let taffy_style = style_map::map_style(computed);

            // 先递归处理子节点
            let child_ids: Vec<NodeId> = node.borrow().child_nodes
                .iter()
                .filter_map(|child| build_node_recursive(tree, child, styles))
                .collect();

            // 创建 taffy 节点
            let taffy_node = tree.taffy.new_leaf_with_children(
                taffy_style,
                child_ids,
            );

            tree.node_map.insert(addr, taffy_node);
            Some(taffy_node)
        }
        _ => None, // Text/Comment/Document 等
    }
}
```

注意：实际 taffy API 可能是 `new_leaf` 或 `new_with_children`，需要根据 taffy 0.7.x 的实际 API 调整。如果 taffy 的 API 是先创建 leaf 再 set_children，则需要调整顺序。

- [ ] **步骤 4：运行测试确认通过**

```powershell
cargo test -p muskitty-layout --test build_tree
```

- [ ] **步骤 5：提交**

```bash
git add crates/muskitty-layout
git commit -m "[layout] L-2: DOM -> LayoutTree conversion (build_layout_tree)"
```

## L-3: ComputedStyle → taffy::Style 映射（box model）

### 目标

实现 `map_style`：将 ComputedStyle 中的 display/width/height/margin/padding/box-sizing 映射到 taffy::Style。

### 文件

- 修改: `crates/muskitty-layout/src/style_map.rs`
- 创建: `crates/muskitty-layout/tests/style_map.rs`

### CSS 属性 → taffy::Style 字段映射表

| CSS 属性 | taffy::Style 字段 | 备注 |
|----------|-------------------|------|
| `display` | `display` (Flex/Grid/Block) | none 在 build 阶段已排除 |
| `width` | `size.width` | auto/length/percentage |
| `height` | `size.height` | auto/length/percentage |
| `min-width` | `min_size.width` | |
| `max-width` | `max_size.width` | |
| `margin-*` | `margin` (top/right/bottom/left) | |
| `padding-*` | `padding` (top/right/bottom/left) | |
| `box-sizing` | `box_size` (BorderBox/ContentBox) | |
| `overflow` | `overflow` | |

### 步骤

- [ ] **步骤 1：写失败测试 — display 映射**

`tests/style_map.rs`:
```rust
use muskitty_layout::style_map::map_style;
use muskitty_cascade::{ComputedStyle, ComputedValue};
use taffy::Style;

#[test]
fn display_block_maps_to_block() {
    let mut cs = ComputedStyle::new();
    cs.set("display", ComputedValue::Keyword("block".to_string()));
    let style = map_style(Some(&cs));
    assert_eq!(style.display, taffy::style::Display::Block);
}

#[test]
fn display_flex_maps_to_flex() {
    let mut cs = ComputedStyle::new();
    cs.set("display", ComputedValue::Keyword("flex".to_string()));
    let style = map_style(Some(&cs));
    assert_eq!(style.display, taffy::style::Display::Flex);
}

#[test]
fn display_grid_maps_to_grid() {
    let mut cs = ComputedStyle::new();
    cs.set("display", ComputedValue::Keyword("grid".to_string()));
    let style = map_style(Some(&cs));
    assert_eq!(style.display, taffy::style::Display::Grid);
}

#[test]
fn no_display_defaults_to_block() {
    let cs = ComputedStyle::new();
    let style = map_style(Some(&cs));
    // CSS initial value for display is inline, but taffy default is Block
    // 实际应根据 registry 的 initial_value
    assert_eq!(style.display, taffy::style::Display::Block);
}
```

- [ ] **步骤 2：写 width/height 映射测试**

```rust
#[test]
fn width_px_maps_to_length() {
    let mut cs = ComputedStyle::new();
    // width: 200px — computed value 为 Resolved([Dimension(200.0, "px")])
    cs.set("width", ComputedValue::Resolved(vec![
        // ComponentValue::PreservedToken(Token::Dimension(200.0, "px"))
        // 需要构造正确的 ComponentValue
    ]));
    let style = map_style(Some(&cs));
    // style.size.width 应为 Length 200.0
}

#[test]
fn width_auto_maps_to_auto() {
    let mut cs = ComputedStyle::new();
    cs.set("width", ComputedValue::Keyword("auto".to_string()));
    let style = map_style(Some(&cs));
    assert_eq!(style.size.width, taffy::style::Size::AUTO);
}

#[test]
fn width_percentage_maps_to_percent() {
    let mut cs = ComputedStyle::new();
    // width: 50% — computed value 保留为 Percentage
    cs.set("width", ComputedValue::Resolved(vec![
        // ComponentValue::PreservedToken(Token::Percentage(50.0))
    ]));
    let style = map_style(Some(&cs));
    // style.size.width 应为 Percent 50%
}
```

- [ ] **步骤 3：写 margin/padding 映射测试**

```rust
#[test]
fn margin_px_maps_correctly() {
    let mut cs = ComputedStyle::new();
    // margin-top: 10px, margin-right: 20px, margin-bottom: 30px, margin-left: 40px
    cs.set("margin-top", /* ... */);
    cs.set("margin-right", /* ... */);
    cs.set("margin-bottom", /* ... */);
    cs.set("margin-left", /* ... */);
    let style = map_style(Some(&cs));
    assert_eq!(style.margin.top, /* Length 10.0 */);
    assert_eq!(style.margin.right, /* Length 20.0 */);
    // ...
}
```

- [ ] **步骤 4：实现 map_style**

在 `style_map.rs` 中实现完整的映射函数。核心逻辑：

```rust
use muskitty_cascade::{ComputedStyle, ComputedValue};
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::{Token, Numeric};
use taffy::style::*;
use taffy::prelude::*;

pub fn map_style(computed: Option<&ComputedStyle>) -> Style {
    let mut style = Style::default();

    let cs = match computed {
        Some(cs) => cs,
        None => return style, // 默认 Style
    };

    // display
    if let Some(ComputedValue::Keyword(kw)) = cs.get("display") {
        style.display = match kw.to_ascii_lowercase().as_str() {
            "flex" => Display::Flex,
            "grid" => Display::Grid,
            "block" => Display::Block,
            _ => Display::Block, // 默认 block
        };
    }

    // width / height
    if let Some(cv) = cs.get("width") {
        style.size.width = map_size(cv);
    }
    if let Some(cv) = cs.get("height") {
        style.size.height = map_size(cv);
    }

    // margin
    style.margin.top = map_length_auto(cs.get("margin-top"));
    style.margin.right = map_length_auto(cs.get("margin-right"));
    style.margin.bottom = map_length_auto(cs.get("margin-bottom"));
    style.margin.left = map_length_auto(cs.get("margin-left"));

    // padding
    style.padding.top = map_length(cs.get("padding-top"));
    style.padding.right = map_length(cs.get("padding-right"));
    style.padding.bottom = map_length(cs.get("padding-bottom"));
    style.padding.left = map_length(cs.get("padding-left"));

    style
}

fn map_size(cv: &ComputedValue) -> taffy::style::Size<taffy::style::LengthPercentageAuto> {
    match cv {
        ComputedValue::Keyword(kw) if kw.eq_ignore_ascii_case("auto") => {
            Size::AUTO
        }
        ComputedValue::Resolved(cvs) | ComputedValue::Raw(cvs) => {
            if let Some(val) = extract_first_dimension(cvs) {
                Size::Length(val)
            } else if let Some(val) = extract_first_percentage(cvs) {
                Size::Percent(val / 100.0)
            } else {
                Size::AUTO
            }
        }
        _ => Size::AUTO,
    }
}

fn map_length_auto(cv: Option<&ComputedValue>) -> taffy::style::LengthPercentageAuto {
    match cv {
        Some(ComputedValue::Keyword(kw)) if kw.eq_ignore_ascii_case("auto") => {
            LengthPercentageAuto::AUTO
        }
        Some(ComputedValue::Resolved(cvs)) | Some(ComputedValue::Raw(cvs)) => {
            if let Some(val) = extract_first_dimension(cvs) {
                LengthPercentageAuto::Length(val)
            } else if let Some(val) = extract_first_percentage(cvs) {
                LengthPercentageAuto::Percent(val / 100.0)
            } else {
                LengthPercentageAuto::AUTO
            }
        }
        _ => LengthPercentageAuto::AUTO,
    }
}

fn map_length(cv: Option<&ComputedValue>) -> taffy::style::LengthPercentage {
    match cv {
        Some(ComputedValue::Resolved(cvs)) | Some(ComputedValue::Raw(cvs)) => {
            if let Some(val) = extract_first_dimension(cvs) {
                LengthPercentage::Length(val)
            } else if let Some(val) = extract_first_percentage(cvs) {
                LengthPercentage::Percent(val / 100.0)
            } else {
                LengthPercentage::ZERO
            }
        }
        _ => LengthPercentage::ZERO,
    }
}

fn extract_first_dimension(cvs: &[ComponentValue]) -> Option<f32> {
    for cv in cvs {
        if let ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) = cv {
            // 只处理 px（其他单位应已在 cascade compute 阶段解析为 px）
            if unit.eq_ignore_ascii_case("px") {
                return Some(numeric.value as f32);
            }
        }
    }
    None
}

fn extract_first_percentage(cvs: &[ComponentValue]) -> Option<f32> {
    for cv in cvs {
        if let ComponentValue::PreservedToken(Token::Percentage(numeric)) = cv {
            return Some(numeric.value as f32);
        }
    }
    None
}
```

- [ ] **步骤 5：运行测试确认通过**

```powershell
cargo test -p muskitty-layout --test style_map
```

- [ ] **步骤 6：提交**

```bash
git add crates/muskitty-layout
git commit -m "[layout] L-3: ComputedStyle -> taffy::Style mapping (display/size/margin/padding)"
```

## L-4: Flexbox 属性映射

### 目标

扩展 `map_style`，添加 Flexbox 相关属性映射。

### 文件

- 修改: `crates/muskitty-layout/src/style_map.rs`
- 修改: `crates/muskitty-layout/tests/style_map.rs`

### CSS 属性 → taffy 字段映射

| CSS 属性 | taffy::Style 字段 | 备注 |
|----------|-------------------|------|
| `flex-direction` | `flex_direction` | row/row-reverse/column/column-reverse |
| `flex-wrap` | `flex_wrap` | nowrap/wrap/wrap-reverse |
| `justify-content` | `justify_content` | flex-start/center/flex-end/space-between/space-around/space-evenly |
| `align-items` | `align_items` | stretch/flex-start/center/flex-end/baseline |
| `align-self` | `align_self` | auto/stretch/flex-start/center/flex-end/baseline |
| `flex-grow` | `flex_grow` | number |
| `flex-shrink` | `flex_shrink` | number |
| `flex-basis` | `flex_basis` | length/percentage/auto |
| `gap` | `gap` | length/percentage |

### 步骤

- [ ] **步骤 1：写 flexbox 属性映射测试**

```rust
#[test]
fn flex_direction_row() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-direction", ComputedValue::Keyword("row".to_string()));
    let style = map_style(Some(&cs));
    assert_eq!(style.flex_direction, taffy::style::FlexDirection::Row);
}

#[test]
fn justify_content_center() {
    let mut cs = ComputedStyle::new();
    cs.set("justify-content", ComputedValue::Keyword("center".to_string()));
    let style = map_style(Some(&cs));
    assert_eq!(style.justify_content, taffy::style::JustifyContent::Center);
}

#[test]
fn flex_grow_number() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-grow", ComputedValue::Resolved(vec![
        // Number(2.0)
    ]));
    let style = map_style(Some(&cs));
    assert_eq!(style.flex_grow, 2.0);
}
```

- [ ] **步骤 2：实现 flexbox 映射**

在 `style_map.rs` 的 `map_style` 中添加 flexbox 属性映射。每个属性都是从 ComputedValue::Keyword 解析字符串到 taffy 枚举。

- [ ] **步骤 3：扩展 registry（可选）**

如果 cascade 的 `registry.rs` 缺少 flexbox 属性，添加：
- `flex-direction` / `flex-wrap` / `justify-content` / `align-items` / `align-self`
- `flex-grow` / `flex-shrink` / `flex-basis`
- `gap` / `row-gap` / `column-gap`

initial_value 取 CSS 规范默认值。inherited 均为 false。percentages 大多为 None。

- [ ] **步骤 4：运行测试 + 提交**

```powershell
cargo test -p muskitty-layout --test style_map
cargo clippy -p muskitty-layout --all-targets -- -D warnings
git commit -m "[layout] L-4: flexbox property mapping (flex-direction/justify/align/gap)"
```

## L-5: 布局计算 + LayoutResult 输出

### 目标

实现 `compute_layout`：调用 taffy 计算布局，提取每个节点的位置/尺寸到 LayoutResult。

### 文件

- 修改: `crates/muskitty-layout/src/result.rs`
- 创建: `crates/muskitty-layout/src/lib.rs`（添加 compute_layout 函数）
- 创建: `crates/muskitty-layout/tests/compute.rs`

### 步骤

- [ ] **步骤 1：写失败测试 — 基本布局计算**

`tests/compute.rs`:
```rust
use muskitty_layout::{build_layout_tree, compute_layout, LayoutResult};
use muskitty_cascade::ComputedStyle;
use muskitty_dom::{Node, NodeKind, Element, Namespace};
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

#[test]
fn single_block_takes_full_width() {
    // div (display:block, width:auto) → 应占满可用宽度
    let root = make_element("div");
    let mut styles = HashMap::new();
    let mut cs = ComputedStyle::new();
    cs.set("display", ComputedValue::Keyword("block".to_string()));
    styles.insert(Rc::as_ptr(&root) as usize, cs);

    let tree = build_layout_tree(&root, &styles);
    let result = compute_layout(&tree, 800.0, 600.0); // viewport 800x600

    let root_layout = result.get(Rc::as_ptr(&root) as usize);
    assert!(root_layout.is_some());
    let root_layout = root_layout.unwrap();
    assert_eq!(root_layout.width, 800.0); // block 占满宽度
}

#[test]
fn fixed_width_element() {
    // div (width: 200px)
    let root = make_element("div");
    let mut styles = HashMap::new();
    let mut cs = ComputedStyle::new();
    cs.set("width", ComputedValue::Resolved(vec![
        // Dimension(200.0, "px")
    ]));
    styles.insert(Rc::as_ptr(&root) as usize, cs);

    let tree = build_layout_tree(&root, &styles);
    let result = compute_layout(&tree, 800.0, 600.0);

    let layout = result.get(Rc::as_ptr(&root) as usize).unwrap();
    assert_eq!(layout.width, 200.0);
}

#[test]
fn flex_row_layouts_children_horizontally() {
    // div (display:flex) > child1 (width:100px) + child2 (width:200px)
    let root = make_element("div");
    let child1 = make_element("div");
    let child2 = make_element("div");
    root.borrow_mut().append_child(Rc::clone(&child1));
    root.borrow_mut().append_child(Rc::clone(&child2));

    let mut styles = HashMap::new();
    let mut root_cs = ComputedStyle::new();
    root_cs.set("display", ComputedValue::Keyword("flex".to_string()));
    root_cs.set("flex-direction", ComputedValue::Keyword("row".to_string()));
    styles.insert(Rc::as_ptr(&root) as usize, root_cs);

    let mut child1_cs = ComputedStyle::new();
    child1_cs.set("width", /* 100px */);
    styles.insert(Rc::as_ptr(&child1) as usize, child1_cs);

    let mut child2_cs = ComputedStyle::new();
    child2_cs.set("width", /* 200px */);
    styles.insert(Rc::as_ptr(&child2) as usize, child2_cs);

    let tree = build_layout_tree(&root, &styles);
    let result = compute_layout(&tree, 800.0, 600.0);

    let c1 = result.get(Rc::as_ptr(&child1) as usize).unwrap();
    let c2 = result.get(Rc::as_ptr(&child2) as usize).unwrap();
    assert_eq!(c1.width, 100.0);
    assert_eq!(c2.width, 200.0);
    // flex row: children 排列在水平方向
    assert_eq!(c1.x, 0.0); // 第一个子元素 x=0
    assert_eq!(c2.x, 100.0); // 第二个子元素 x=100（紧接第一个）
}
```

- [ ] **步骤 2：实现 compute_layout**

在 `lib.rs` 或新模块中：

```rust
/// 计算布局树，返回每个元素的布局结果。
///
/// `viewport_width` / `viewport_height` 为根容器的可用空间（px）。
pub fn compute_layout(
    tree: &LayoutTree,
    viewport_width: f32,
    viewport_height: f32,
) -> LayoutResult {
    let mut result = LayoutResult::new();

    if let Some(root) = tree.root {
        // 调用 taffy 计算布局
        tree.taffy.compute_layout_with_measure(
            root,
            taffy::geometry::Size {
                width: taffy::style::AvailableSpace::Definite(viewport_width),
                height: taffy::style::AvailableSpace::Definite(viewport_height),
            },
            // measure function: 对于文本节点等 leaf 节点，
            // 返回 zero size（本批次不实现文本测量）
            |_, _, _, _| taffy::geometry::Size::ZERO,
        ).expect("layout computation failed");

        // 遍历所有节点，提取布局结果
        for (&dom_addr, &taffy_node) in &tree.node_map {
            let layout = tree.taffy.layout(taffy_node).expect("layout not found");
            result.nodes.insert(dom_addr, NodeLayout {
                x: layout.location.x,
                y: layout.location.y,
                width: layout.size.width,
                height: layout.size.height,
            });
        }
    }

    result
}
```

注意：taffy 的 `compute_layout_with_measure` 签名可能略有不同，需根据 0.7.x 实际 API 调整。也可能用 `compute_layout` (不带 measure 的版本) 先做。

- [ ] **步骤 3：运行测试确认通过**

```powershell
cargo test -p muskitty-layout --test compute
```

- [ ] **步骤 4：提交**

```bash
git add crates/muskitty-layout
git commit -m "[layout] L-5: layout computation + LayoutResult extraction"
```

## L-6: 端到端集成测试

### 目标

完整 pipeline 测试：HTML + CSS → cascade → computed style → layout tree → layout result。

### 文件

- 创建: `crates/muskitty-layout/tests/integration.rs`

### 步骤

- [ ] **步骤 1：写完整 pipeline 测试**

```rust
use muskitty_layout::{build_layout_tree, compute_layout};
use muskitty_cascade::{collect_declared_values, cascade_for_element, cascade_winner, apply_defaulting, compute_value, ComputeContext, ComputedStyle, ComputedValue};
use muskitty_cssom::{CssStyleSheet, CssRule, CssStyleRule, CssDeclaration, Origin};
use muskitty_css::parse_stylesheet;
use muskitty_dom::{Node, NodeKind, Element, Namespace};
use muskitty_selectors::matching::DomElement;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

/// 完整 pipeline: DOM + CSS text → computed styles → layout → result
fn full_pipeline(html: &str, css: &str, viewport_w: f32, viewport_h: f32) -> LayoutResult {
    // 1. 解析 HTML → DOM 树 (使用 muskitty-html5-parser)
    // 2. 解析 CSS → Stylesheet → CssStyleSheet
    // 3. 遍历 DOM，对每个元素做 cascade → ComputedStyle
    // 4. build_layout_tree(dom, &computed_styles)
    // 5. compute_layout(tree, viewport_w, viewport_h)
    todo!()
}

#[test]
fn simple_block_layout() {
    // <div><p>Hello</p></div> + div { width: 300px; }
    let result = full_pipeline(
        "<div><p>Hello</p></div>",
        "div { width: 300px; }",
        800.0, 600.0
    );
    // 验证 div 的 width = 300px
    // 验证 p 的 width = 300px（block 子元素占满父宽度）
}

#[test]
fn flex_layout_children_positioned() {
    // <div style="display:flex"><div style="width:100px"></div><div style="width:200px"></div></div>
    let result = full_pipeline(
        "<div style='display:flex'><div style='width:100px'></div><div style='width:200px'></div></div>",
        "",
        800.0, 600.0
    );
    // 验证两个子元素水平排列
    // child1.x == 0, child2.x == 100
}

#[test]
fn percentage_width_resolved() {
    // div(width:50%) in a 800px viewport → width == 400
    let result = full_pipeline(
        "<div style='width:50%'></div>",
        "",
        800.0, 600.0
    );
    // 验证 width == 400.0
}

#[test]
fn margin_applied_to_position() {
    // div(margin-left:20px, width:100px) → x == 20
    let result = full_pipeline(
        "<div style='margin-left:20px; width:100px'></div>",
        "",
        800.0, 600.0
    );
    // 验证 x == 20.0
}

#[test]
fn nested_block_layout() {
    // div > div > div, 每层 padding:10px
    // 验证嵌套的 x/y offset 正确传递
}
```

- [ ] **步骤 2：实现 full_pipeline 辅助函数**

这个函数需要串联所有之前的模块：
1. `muskitty_html5_parser::parse(html)` → DOM 树
2. `muskitty_css::parse_stylesheet(css)` → Stylesheet → `CssStyleSheet::from(stylesheet)`
3. 遍历 DOM 树，对每个元素：
   - `collect_declared_values(&element, &sheets)`
   - `cascade_for_element(&declared)`
   - `cascade_winner(&group)` for each property
   - `apply_defaulting(property, cascaded, parent_computed)`
   - `compute_value(property, specified, &ctx)`
   - 组装 `ComputedStyle`
4. `build_layout_tree(&dom_root, &computed_styles)`
5. `compute_layout(&tree, viewport_w, viewport_h)`

注意：需要递归遍历 DOM 树计算每个元素的 ComputedStyle，因为继承属性需要 parent 的 computed value。

- [ ] **步骤 3：运行测试 + 修复**

```powershell
cargo test -p muskitty-layout
```

- [ ] **步骤 4：全 workspace 回归**

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

- [ ] **步骤 5：更新 PROGRESS.md**

在 PROGRESS.md 中添加 Phase 3 状态条目。

- [ ] **步骤 6：提交**

```bash
git add crates/muskitty-layout PROGRESS.md
git commit -m "[layout] L-6: end-to-end integration tests + PROGRESS.md update"
```

## 延后项

- **文本测量**：text 节点目前跳过（不创建 taffy leaf）。需要引入文本布局库（如 `cosmic-text` 或 `swash`）实现 measure function。推迟到 L-7 或独立批次。
- **CSS Grid 完整属性**：grid-template-columns/rows, grid-area, grid-gap 等映射。推迟。
- **Float 布局**：taffy 支持 float_layout feature，但映射逻辑推迟。
- **Position: absolute/relative/fixed**：定位元素映射推迟。
- **Overflow / scroll**：推迟。
- **多字体 / @font-face**：推迟到 Renderer 层。
- **Writing modes (vertical-rl 等)**：推迟。
- **Table layout**：taffy 不支持，需要自行实现或推迟。
- **百分比解析在 layout 阶段**：width/height/margin/padding 的百分比当前透传到 taffy，taffy 内部处理。但部分百分比的 containing block 依赖布局结果（如 height 百分比基于父元素 height），需要验证 taffy 是否正确处理。

## 质量门禁

每个批次 commit 前依次执行：

```powershell
cargo fmt -p muskitty-layout -- --check
cargo test -p muskitty-layout
cargo check -p muskitty-layout
cargo clippy -p muskitty-layout --all-targets -- -D warnings
```

L-6 额外执行全 workspace 回归：

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
