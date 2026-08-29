//! chrome 命中测试与事件应用（纯函数，无窗口依赖）。
//!
//! [`hit_test`] 把物理像素坐标映射为 [`ChromeHit`]；[`apply_mouse`] /
//! [`apply_key`] 把输入应用到 [`ChromeState`] 并产出 [`ChromeEffect`]，
//! 由 `App`（winit 侧）执行——与 W-5 的"事件处理只产生效果，渲染统一
//! flush"分层一致。

use crate::chrome::model::{ChromeHit, ChromeRects, ChromeState};

/// chrome 键（winit 事件在 App 侧转换为该枚举；纯函数无外部类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeKey {
    /// 字符输入（地址栏聚焦时插入末尾）。
    Char(char),
    /// 退格（删除末尾字符）。
    Backspace,
    /// 回车（提交地址栏）。
    Enter,
    /// Esc（地址栏聚焦时取消焦点；未聚焦时由 App 处理关窗）。
    Escape,
}

/// chrome 事件的应用效果（App 执行；状态变更已就地写入 `ChromeState`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChromeEffect {
    /// 无效果（但仍需重绘：hover/focus 变化）。
    Repaint,
    /// 切到第 `n` 个标签。
    SwitchTab(usize),
    /// 关闭第 `n` 个标签。
    CloseTab(usize),
    /// 新建标签。
    NewTab,
    /// 刷新页面。
    ReloadPage,
    /// 地址栏提交（当前地址栏文本）。
    UrlSubmitted(String),
}

/// 命中测试（物理像素坐标 → chrome 元素）。
///
/// 命中优先级：关闭按钮 > 标签（同一标签区域内 × 优先）；chrome 之外
/// 归页面视口。
pub fn hit_test(rects: &ChromeRects, x: f32, y: f32) -> ChromeHit {
    if y < rects.tab_strip.height {
        for (i, close) in rects.tab_close_buttons.iter().enumerate().rev() {
            if close.contains(x, y) {
                return ChromeHit::TabClose(i);
            }
        }
        for (i, tab) in rects.tabs.iter().enumerate().rev() {
            if tab.contains(x, y) {
                return ChromeHit::Tab(i);
            }
        }
        if rects.new_tab_button.contains(x, y) {
            return ChromeHit::NewTab;
        }
        return ChromeHit::PageViewport; // 标签条空白处不命中元素
    }
    if y < rects.toolbar.y + rects.toolbar.height {
        if rects.back_button.contains(x, y) {
            return ChromeHit::Back;
        }
        if rects.forward_button.contains(x, y) {
            return ChromeHit::Forward;
        }
        if rects.reload_button.contains(x, y) {
            return ChromeHit::Reload;
        }
        if rects.address_bar.contains(x, y) {
            return ChromeHit::AddressBar;
        }
        return ChromeHit::PageViewport;
    }
    ChromeHit::PageViewport
}

/// 应用鼠标按下（先更新 hover/focus 状态，返回需执行的效果）。
pub fn apply_mouse(state: &mut ChromeState, rects: &ChromeRects, x: f32, y: f32) -> ChromeEffect {
    let hit = hit_test(rects, x, y);
    state.hover = Some(hit);
    match hit {
        ChromeHit::Tab(i) => {
            state.address_focused = false;
            ChromeEffect::SwitchTab(i)
        }
        ChromeHit::TabClose(i) => {
            state.address_focused = false;
            ChromeEffect::CloseTab(i)
        }
        ChromeHit::NewTab => {
            state.address_focused = false;
            state.address_text.clear();
            ChromeEffect::NewTab
        }
        ChromeHit::Reload => ChromeEffect::ReloadPage,
        ChromeHit::AddressBar => {
            state.address_focused = true;
            ChromeEffect::Repaint
        }
        // 空白处/页面视口点击：取消地址栏焦点（与浏览器一致）。
        ChromeHit::Back | ChromeHit::Forward | ChromeHit::PageViewport => {
            state.address_focused = false;
            ChromeEffect::Repaint
        }
    }
}

/// hover 变化（MouseMoved；hover 变化时需要重绘按钮/标签 hover 态）。
pub fn apply_hover(state: &mut ChromeState, rects: &ChromeRects, x: f32, y: f32) -> bool {
    let hit = hit_test(rects, x, y);
    let hover_hit = match hit {
        ChromeHit::Tab(i) => Some(ChromeHit::Tab(i)),
        ChromeHit::NewTab | ChromeHit::Reload => Some(hit),
        _ => None,
    };
    let changed = state.hover != hover_hit;
    state.hover = hover_hit;
    changed
}

/// 应用键盘输入（地址栏；返回效果）。
pub fn apply_key(state: &mut ChromeState, key: ChromeKey) -> ChromeEffect {
    match key {
        ChromeKey::Char(c) if state.address_focused => {
            state.address_text.push(c);
            ChromeEffect::Repaint
        }
        ChromeKey::Backspace if state.address_focused => {
            state.address_text.pop();
            ChromeEffect::Repaint
        }
        ChromeKey::Enter if state.address_focused => {
            let url = state.address_text.trim().to_string();
            state.address_focused = false;
            ChromeEffect::UrlSubmitted(url)
        }
        ChromeKey::Escape if state.address_focused => {
            state.address_focused = false;
            ChromeEffect::Repaint
        }
        _ => ChromeEffect::Repaint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::model::layout_chrome;

    fn rects() -> ChromeRects {
        layout_chrome(800, 600, 1.0, 2, &ChromeState::default())
    }

    #[test]
    fn hit_test_tabs_and_close_priority() {
        let r = rects();
        // 第 0 个标签内部（避开右上角 ×）。
        let t = r.tabs[0];
        assert_eq!(hit_test(&r, t.x + 5.0, t.y + 5.0), ChromeHit::Tab(0));
        // 关闭按钮优先于标签（同区域右上角）。
        let c = r.tab_close_buttons[0];
        assert_eq!(
            hit_test(&r, c.x + c.width / 2.0, c.y + c.height / 2.0),
            ChromeHit::TabClose(0)
        );
    }

    #[test]
    fn hit_test_buttons_and_address_bar() {
        let r = rects();
        let center = |rect: &crate::chrome::model::Rect| {
            (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
        };
        assert_eq!(
            hit_test(&r, center(&r.new_tab_button).0, center(&r.new_tab_button).1),
            ChromeHit::NewTab
        );
        assert_eq!(
            hit_test(&r, center(&r.back_button).0, center(&r.back_button).1),
            ChromeHit::Back
        );
        assert_eq!(
            hit_test(&r, center(&r.forward_button).0, center(&r.forward_button).1),
            ChromeHit::Forward
        );
        assert_eq!(
            hit_test(&r, center(&r.reload_button).0, center(&r.reload_button).1),
            ChromeHit::Reload
        );
        assert_eq!(
            hit_test(&r, center(&r.address_bar).0, center(&r.address_bar).1),
            ChromeHit::AddressBar
        );
    }

    #[test]
    fn hit_test_strip_blank_and_page_viewport() {
        let r = rects();
        // 标签条空白（标签右侧与 + 按钮之间）。
        let blank_x =
            (r.tabs.last().unwrap().x + r.tabs.last().unwrap().width + r.new_tab_button.x) / 2.0;
        assert_eq!(hit_test(&r, blank_x, 10.0), ChromeHit::PageViewport);
        // 页面视口（chrome 之下）。
        assert_eq!(
            hit_test(&r, 400.0, r.page_viewport.y + 10.0),
            ChromeHit::PageViewport
        );
        // 工具栏空白（后退按钮左侧的空隙）也归页面视口。
        assert_eq!(
            hit_test(&r, 0.0, r.toolbar.y + 2.0),
            ChromeHit::PageViewport
        );
        let b = &r.back_button;
        assert_eq!(hit_test(&r, b.x + 2.0, b.y + 2.0), ChromeHit::Back);
    }

    #[test]
    fn apply_mouse_switch_close_new_reload_focus() {
        let r = rects();
        // 点标签 1 → 切换。
        let mut s = ChromeState::default();
        let t = &r.tabs[1];
        assert_eq!(
            apply_mouse(&mut s, &r, t.x + 5.0, t.y + 5.0),
            ChromeEffect::SwitchTab(1)
        );
        assert!(!s.address_focused);
        // 点关闭 0。
        let c = &r.tab_close_buttons[0];
        assert_eq!(
            apply_mouse(&mut s, &r, c.x + 2.0, c.y + 2.0),
            ChromeEffect::CloseTab(0)
        );
        // 点 + → 新建（地址栏清空）。
        s.address_text = "leftover".into();
        let n = &r.new_tab_button;
        assert_eq!(
            apply_mouse(&mut s, &r, n.x + n.width / 2.0, n.y + n.height / 2.0),
            ChromeEffect::NewTab
        );
        assert!(s.address_text.is_empty());
        // 点刷新。
        let rl = &r.reload_button;
        assert_eq!(
            apply_mouse(&mut s, &r, rl.x + 4.0, rl.y + 4.0),
            ChromeEffect::ReloadPage
        );
        // 点地址栏 → 聚焦。
        let a = &r.address_bar;
        assert_eq!(
            apply_mouse(&mut s, &r, a.x + a.width / 2.0, a.y + a.height / 2.0),
            ChromeEffect::Repaint
        );
        assert!(s.address_focused);
        // 点页面视口 → 取消聚焦。
        assert_eq!(
            apply_mouse(&mut s, &r, 400.0, r.page_viewport.y + 10.0),
            ChromeEffect::Repaint
        );
        assert!(!s.address_focused);
    }

    #[test]
    fn apply_key_typing_backspace_enter_escape() {
        let mut s = ChromeState {
            address_focused: true,
            ..ChromeState::default()
        };
        assert_eq!(
            apply_key(&mut s, ChromeKey::Char('h')),
            ChromeEffect::Repaint
        );
        apply_key(&mut s, ChromeKey::Char('i'));
        assert_eq!(s.address_text, "hi");
        apply_key(&mut s, ChromeKey::Backspace);
        assert_eq!(s.address_text, "h");
        // 回车提交：清焦点，返回 UrlSubmitted。
        s.address_text = "rust.org".into();
        assert_eq!(
            apply_key(&mut s, ChromeKey::Enter),
            ChromeEffect::UrlSubmitted("rust.org".into())
        );
        assert!(!s.address_focused);
        // 未聚焦时字符不插入（提交后文本保留，与浏览器地址栏一致）、回车不重复提交。
        s.address_focused = false;
        assert_eq!(
            apply_key(&mut s, ChromeKey::Char('x')),
            ChromeEffect::Repaint
        );
        assert_eq!(s.address_text, "rust.org");
        assert_eq!(apply_key(&mut s, ChromeKey::Enter), ChromeEffect::Repaint);
        // 聚焦时 Esc 只取消焦点。
        s.address_focused = true;
        assert_eq!(apply_key(&mut s, ChromeKey::Escape), ChromeEffect::Repaint);
        assert!(!s.address_focused);
    }

    #[test]
    fn apply_hover_tracks_interactive_only() {
        let r = rects();
        let mut s = ChromeState::default();
        // hover 标签 → 记录。
        let t = &r.tabs[0];
        assert!(apply_hover(&mut s, &r, t.x + 5.0, t.y + 5.0));
        assert_eq!(s.hover, Some(ChromeHit::Tab(0)));
        // hover 地址栏 → 不记录（无 hover 视觉态）。
        let a = &r.address_bar;
        assert!(apply_hover(&mut s, &r, a.x + 10.0, a.y + 5.0));
        assert_eq!(s.hover, None);
        // hover 不变 → 返回 false（无需重绘）。
        assert!(!apply_hover(&mut s, &r, a.x + 11.0, a.y + 6.0));
    }
}
