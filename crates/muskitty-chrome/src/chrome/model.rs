//! chrome 布局模型（纯函数，无窗口依赖）。
//!
//! [`layout_chrome`] 把窗口几何 + 标签数换算成 chrome 各元素的物理像素
//! 矩形（[`ChromeRects`]），供绘制（[`crate::chrome::paint`]）与命中测试
//! （[`crate::chrome::input`]）共用——布局与命中测试读同一份矩形，是
//! Chromium Views"布局/绘制/命中分层"的最小同构。
//!
//! 视觉规格取 Chromium light 基线（逻辑 px，物理 = 逻辑 × `scale`）：
//! 标签条 36 高 / 工具栏 44 高 / 标签宽 `min(220, 可用宽/n)` / 地址栏
//! 高 30。本模块零外部依赖类型，`--no-default-features` 下照常编译。

/// chrome 自身元素上的命中结果（物理像素坐标点 → 元素）。
///
/// 由 [`crate::chrome::input::hit_test`] 产生，`App` 据此分发（页面
/// 视口内的事件不在本轮消费——页面命中测试延后）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeHit {
    /// 标签（点击切换）。
    Tab(usize),
    /// 标签关闭按钮（点击关闭该标签）。
    TabClose(usize),
    /// 新建标签按钮。
    NewTab,
    /// 后退按钮（历史栈未建，v1 禁用态）。
    Back,
    /// 前进按钮（同上）。
    Forward,
    /// 刷新按钮。
    Reload,
    /// 地址栏（点击聚焦）。
    AddressBar,
    /// 页面视口（chrome 之外的页面区域）。
    PageViewport,
}

/// chrome 可交互状态（地址栏 + hover）。
///
/// 标签列表本身由标签集合（webview）持有，不在此重复；本结构只含
/// chrome 自身 UI 状态。v1 光标恒在文本末尾（无光标移动/选区，见 ADR）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChromeState {
    /// 地址栏文本。
    pub address_text: String,
    /// 地址栏是否聚焦（聚焦时白底描边 + 显示光标）。
    pub address_focused: bool,
    /// 当前 hover 的元素（绘制 hover 态；None = 无）。
    pub hover: Option<ChromeHit>,
}

/// 物理像素矩形（chrome 元素，窗口坐标系，原点在窗口客户区左上角）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 点是否在矩形内（右/下边界开区间）。
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// chrome 布局结果：各元素物理像素矩形。
///
/// `tabs` / `tab_close_buttons` 按标签索引对齐（等长）。所有矩形都在
/// 窗口客户区内、互不重叠（页面视口之外）。
#[derive(Debug, Clone, PartialEq)]
pub struct ChromeRects {
    /// 标签条整条。
    pub tab_strip: Rect,
    /// 每个标签的矩形（含其关闭按钮）。
    pub tabs: Vec<Rect>,
    /// 每个标签的关闭按钮矩形。
    pub tab_close_buttons: Vec<Rect>,
    /// 新建标签按钮。
    pub new_tab_button: Rect,
    /// 工具栏整条。
    pub toolbar: Rect,
    /// 后退按钮。
    pub back_button: Rect,
    /// 前进按钮。
    pub forward_button: Rect,
    /// 刷新按钮。
    pub reload_button: Rect,
    /// 地址栏。
    pub address_bar: Rect,
    /// 页面视口（chrome 之下的页面区域）。
    pub page_viewport: Rect,
    /// 布局用缩放因子（物理 = 逻辑 × scale，供绘制文本缩放）。
    pub scale: f32,
}

/// 逻辑规格（物理 = 逻辑 × scale）。
const TAB_STRIP_H: f32 = 36.0;
const TOOLBAR_H: f32 = 44.0;
const TAB_HEIGHT: f32 = 30.0;
const TAB_TOP: f32 = 6.0;
const TAB_MAX_W: f32 = 220.0;
const TAB_MIN_W: f32 = 48.0;
const TAB_GAP: f32 = 2.0;
const TAB_STRIP_PAD: f32 = 8.0;
const NEW_TAB_SIZE: f32 = 28.0;
const BUTTON_SIZE: f32 = 32.0;
const BUTTON_GAP: f32 = 4.0;
const ADDRESS_HEIGHT: f32 = 30.0;
const ADDRESS_MARGIN: f32 = 8.0;

/// chrome 总高度（物理 px）：标签条 + 工具栏。
pub fn chrome_height(scale: f32) -> f32 {
    (TAB_STRIP_H + TOOLBAR_H) * scale
}

/// 布局 chrome（纯函数）。
///
/// `width`/`height` 为窗口客户区**物理**尺寸，`scale` 为 HiDPI 因子，
/// `tab_count` 为当前标签数（≥1）。标签从左往右排，关闭按钮贴标签右侧
/// 内边；标签过宽截到 [`TAB_MAX_W`]、过窄缩到 [`TAB_MIN_W`]（不保证
/// 不重叠的极端窄窗口下按最小宽排布，可能溢出——可接受，v1 不做滚动）。
pub fn layout_chrome(
    width: u32,
    height: u32,
    scale: f32,
    tab_count: usize,
    _state: &ChromeState,
) -> ChromeRects {
    let s = scale;
    let w = width as f32;
    let h = height as f32;
    let strip_h = TAB_STRIP_H * s;
    let toolbar_h = TOOLBAR_H * s;
    let chrome_h = strip_h + toolbar_h;

    let tab_strip = Rect::new(0.0, 0.0, w, strip_h);

    // 新建按钮：标签条右侧（右缘留 TAB_STRIP_PAD）。
    let new_tab_size = NEW_TAB_SIZE * s;
    let new_tab_button = Rect::new(
        w - TAB_STRIP_PAD * s - new_tab_size,
        (strip_h - new_tab_size) / 2.0,
        new_tab_size,
        new_tab_size,
    );

    // 标签：左起 TAB_STRIP_PAD，等宽，宽 = clamp(可用/n, min, max)。
    let pad = TAB_STRIP_PAD * s;
    let gap = TAB_GAP * s;
    let tab_h = TAB_HEIGHT * s;
    let tab_y = TAB_TOP * s;
    let close_size = 16.0 * s;
    // 标签可用总宽 = 左 pad 到新建按钮左缘之间的空间。
    let usable = (new_tab_button.x - pad - gap).max(0.0);
    let tab_w = if tab_count == 0 {
        0.0
    } else {
        ((usable - (tab_count - 1) as f32 * gap) / tab_count as f32)
            .clamp(TAB_MIN_W * s, TAB_MAX_W * s)
    };
    let mut tabs = Vec::with_capacity(tab_count);
    let mut closes = Vec::with_capacity(tab_count);
    for i in 0..tab_count {
        let x = pad + i as f32 * (tab_w + gap);
        tabs.push(Rect::new(x, tab_y, tab_w, tab_h));
        // 关闭按钮：标签右上角内边。
        closes.push(Rect::new(
            x + tab_w - close_size - 4.0 * s,
            tab_y + (tab_h - close_size) / 2.0,
            close_size,
            close_size,
        ));
    }

    // 工具栏 + 按钮（导航三键靠左，地址栏占其余宽度）。
    let toolbar = Rect::new(0.0, strip_h, w, toolbar_h);
    let btn = BUTTON_SIZE * s;
    let btn_y = strip_h + (toolbar_h - btn) / 2.0;
    let btn_gap = BUTTON_GAP * s;
    let back = Rect::new(BUTTON_GAP * s, btn_y, btn, btn);
    let forward = Rect::new(back.x + btn + btn_gap, btn_y, btn, btn);
    let reload = Rect::new(forward.x + btn + btn_gap, btn_y, btn, btn);
    let addr_h = ADDRESS_HEIGHT * s;
    let addr_y = strip_h + (toolbar_h - addr_h) / 2.0;
    let address_bar = Rect::new(
        reload.x + btn + ADDRESS_MARGIN * s,
        addr_y,
        (w - (reload.x + btn + ADDRESS_MARGIN * s) - ADDRESS_MARGIN * s).max(0.0),
        addr_h,
    );

    let page_viewport = Rect::new(0.0, chrome_h, w, (h - chrome_h).max(0.0));

    ChromeRects {
        tab_strip,
        tabs,
        tab_close_buttons: closes,
        new_tab_button,
        toolbar,
        back_button: back,
        forward_button: forward,
        reload_button: reload,
        address_bar,
        page_viewport,
        scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(w: u32, h: u32, scale: f32, tabs: usize) -> ChromeRects {
        layout_chrome(w, h, scale, tabs, &ChromeState::default())
    }

    #[test]
    fn default_geometry_at_scale_1() {
        let r = layout(800, 600, 1.0, 1);
        assert_eq!(r.tab_strip, Rect::new(0.0, 0.0, 800.0, 36.0));
        assert_eq!(r.toolbar, Rect::new(0.0, 36.0, 800.0, 44.0));
        assert_eq!(chrome_height(1.0), 80.0);
        // 页面视口从 chrome 底到窗口底。
        assert_eq!(r.page_viewport, Rect::new(0.0, 80.0, 800.0, 520.0));
    }

    #[test]
    fn all_rects_scale_linearly() {
        let r = layout(1600, 1200, 2.0, 1);
        assert_eq!(r.tab_strip.height, 72.0);
        assert_eq!(r.toolbar.y, 72.0);
        assert_eq!(r.address_bar.height, 60.0);
        assert_eq!(r.page_viewport.y, 160.0);
        assert_eq!(r.page_viewport.height, 1040.0);
    }

    #[test]
    fn single_tab_uses_max_width() {
        let r = layout(800, 600, 1.0, 1);
        assert_eq!(r.tabs.len(), 1);
        assert_eq!(r.tabs[0].width, TAB_MAX_W);
        assert_eq!(r.tabs[0].y, TAB_TOP);
        assert_eq!(r.tabs[0].height, TAB_HEIGHT);
    }

    #[test]
    fn tabs_equal_width_no_overlap() {
        let r = layout(800, 600, 1.0, 3);
        assert_eq!(r.tabs.len(), 3);
        for pair in r.tabs.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(b.x >= a.x + a.width, "tabs must not overlap: {a:?} {b:?}");
            assert!((b.x - (a.x + a.width) - TAB_GAP).abs() < 0.01);
        }
        // 每个关闭按钮在对应标签内部。
        for (tab, close) in r.tabs.iter().zip(&r.tab_close_buttons) {
            assert!(close.x >= tab.x && close.x + close.width <= tab.x + tab.width);
            assert!(close.y >= tab.y && close.y + close.height <= tab.y + tab.height);
        }
    }

    #[test]
    fn many_tabs_clamp_to_min_width() {
        let r = layout(300, 600, 1.0, 20);
        for t in &r.tabs {
            assert_eq!(t.width, TAB_MIN_W);
        }
    }

    #[test]
    fn new_tab_button_right_of_tabs_within_strip() {
        let r = layout(800, 600, 1.0, 2);
        let last = r.tabs.last().unwrap();
        assert!(r.new_tab_button.x >= last.x + last.width);
        assert!(
            r.new_tab_button.y >= 0.0
                && r.new_tab_button.y + r.new_tab_button.height <= r.tab_strip.height
        );
    }

    #[test]
    fn nav_buttons_in_toolbar_address_bar_fills_rest() {
        let r = layout(800, 600, 1.0, 1);
        for b in [&r.back_button, &r.forward_button, &r.reload_button] {
            assert!(b.y >= r.toolbar.y && b.y + b.height <= r.toolbar.y + r.toolbar.height);
            assert_eq!(b.width, BUTTON_SIZE);
        }
        assert!(r.forward_button.x > r.back_button.x);
        assert!(r.reload_button.x > r.forward_button.x);
        assert_eq!(
            r.address_bar.x,
            r.reload_button.x + r.reload_button.width + ADDRESS_MARGIN
        );
        assert_eq!(
            r.address_bar.x + r.address_bar.width,
            800.0 - ADDRESS_MARGIN
        );
        assert!(r.address_bar.y >= r.toolbar.y);
        assert!(r.address_bar.y + r.address_bar.height <= r.toolbar.y + r.toolbar.height);
    }

    #[test]
    fn rect_contains_boundaries() {
        let rect = Rect::new(10.0, 10.0, 50.0, 20.0);
        assert!(rect.contains(10.0, 10.0));
        assert!(rect.contains(59.9, 29.9));
        assert!(!rect.contains(60.0, 15.0));
        assert!(!rect.contains(15.0, 30.0));
        assert!(!rect.contains(5.0, 5.0));
    }

    #[test]
    fn viewport_never_negative_height() {
        let r = layout(800, 40, 1.0, 1); // 窗口比 chrome 还矮
        assert_eq!(r.page_viewport.height, 0.0);
    }
}
