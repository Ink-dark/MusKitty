//! WebView 与标签集合（W-5）。
//!
//! [`WebView`] 是一份页面的全部状态：内容（HTML + CSS）、渲染输出与
//! 布局状态（脏检查用）、以及两个脏位（[`WebView::needs_repaint`] /
//! [`WebView::close_scheduled`]，Servo §1.7 延迟更新模式——事件处理只
//! 标记，事件循环的统一 flush 点才真正重渲染 / 移除）。
//!
//! [`WebViewCollection`] 管理多份 [`WebView`]（标签）与 active 索引：
//! 新建 / 关闭 / 切换标签后保证 `active` 指向合法视图，且 active 变化
//! 时自动标记 [`WebView::needs_repaint`]（切换必须刷新显示）。
//!
//! 本模块纯状态、零外部依赖、不依赖 winit（app.rs 是 winit 后端的
//! 消费方），`--no-default-features` 下照常编译与测试（对齐
//! `docs/decisions/2026-08-16-external-dependency-decoupling.md`）。
//! 规划见 `docs/plans/2026-08-23-windowing.md` §W-5。

/// 一份页面（一个标签）的全部状态。
///
/// `pixels` / `width` / `height` 为最近一次渲染的 RGBA 输出（物理
/// 分辨率，present 用）；`logical_width` / `logical_height` / `scale`
/// 为最近一次渲染的布局状态（脏检查用，避免每帧全量渲染）。
/// `html` / `css` 为页面内容（持有 `String`：标签内容需可区分/可变，
/// 由导航（`crate::navigation`）或文件加载填充）。
#[derive(Debug, Clone)]
pub struct WebView {
    /// 页面 HTML。
    pub html: String,
    /// 页面 CSS。
    pub css: String,
    /// 标签标题（chrome 标签栏显示；导航提交后先更新为 URL，到站后为
    /// 最终 URL / 文件名）。
    pub title: String,
    /// 导航代数：本标签每发起一次地址栏导航 +1；到站结果只有代数匹配
    /// 才应用——用户改址 / 关签后索引复用导致的过期导航静默丢弃
    /// （结果携带提交时的 `(tab, epoch)` 快照）。
    pub navigation_epoch: u64,
    /// 脏位：需要重渲染（内容/尺寸/scale 变化或显式 reload）。
    needs_repaint: bool,
    /// 脏位：已请求关闭（统一 flush 点移除，Servo §1.7 延迟更新）。
    close_scheduled: bool,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    logical_width: u32,
    logical_height: u32,
    scale: f32,
}

impl WebView {
    /// 构造 WebView：尚未渲染（`needs_repaint = true`，首个 flush 点
    /// 渲染），布局状态为零值。
    pub fn new(html: impl Into<String>, css: impl Into<String>) -> Self {
        Self {
            html: html.into(),
            css: css.into(),
            title: String::from("新标签页"),
            navigation_epoch: 0,
            needs_repaint: true,
            close_scheduled: false,
            pixels: Vec::new(),
            width: 0,
            height: 0,
            logical_width: 0,
            logical_height: 0,
            scale: 1.0,
        }
    }

    /// 更新标签标题。
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// 标记需要重渲染（reload / 标签切换 / 内容变化）。
    pub fn mark_needs_repaint(&mut self) {
        self.needs_repaint = true;
    }

    /// 标记已请求关闭；真正移除在统一 flush 点（[`WebViewCollection::flush_close`]）。
    pub fn mark_close_scheduled(&mut self) {
        self.close_scheduled = true;
    }

    /// 是否需要重渲染。
    pub fn needs_repaint(&self) -> bool {
        self.needs_repaint
    }

    /// 是否已请求关闭。
    pub fn close_scheduled(&self) -> bool {
        self.close_scheduled
    }

    /// 最近一次渲染的布局状态 `(logical_width, logical_height, scale)`。
    pub fn layout_state(&self) -> (u32, u32, f32) {
        (self.logical_width, self.logical_height, self.scale)
    }

    /// 以逻辑尺寸 + scale 重新渲染并保存输出（由 app 层驱动
    /// `page::render_page`，本方法只存结果）。
    pub fn store_render(
        &mut self,
        pixels: Vec<u8>,
        width: u32,
        height: u32,
        logical_width: u32,
        logical_height: u32,
        scale: f32,
    ) {
        self.pixels = pixels;
        self.width = width;
        self.height = height;
        self.logical_width = logical_width;
        self.logical_height = logical_height;
        self.scale = scale;
        self.needs_repaint = false;
    }

    /// 最近一帧 `(pixels, width, height)`（RGBA8，物理分辨率）。
    pub fn frame(&self) -> (&[u8], u32, u32) {
        (&self.pixels, self.width, self.height)
    }
}

/// 多标签集合：视图列表 + active 索引。
///
/// 不变量：`views` 非空时 `active < views.len()`。所有变更操作
/// （新建/关闭/切换）维护该不变量；active 指向的视图变化时自动
/// [`WebView::mark_needs_repaint`]。
#[derive(Debug, Clone)]
pub struct WebViewCollection {
    views: Vec<WebView>,
    active: usize,
}

impl WebViewCollection {
    /// 构造集合：以给定内容创建第一个标签并激活。
    pub fn new(html: impl Into<String>, css: impl Into<String>) -> Self {
        Self {
            views: vec![WebView::new(html, css)],
            active: 0,
        }
    }

    /// 新建标签（内容 `html`/`css`）并激活。
    pub fn new_tab(&mut self, html: impl Into<String>, css: impl Into<String>) {
        self.views.push(WebView::new(html, css));
        self.active = self.views.len() - 1;
    }

    /// 标记 active 标签为待关闭（延迟到 [`Self::flush_close`]）。
    pub fn close_active(&mut self) {
        self.views[self.active].mark_close_scheduled();
    }

    /// 移除所有 `close_scheduled` 的标签，返回移除数。
    ///
    /// active 重定位：`active -= 左侧被移除数`；旧 active 自身被移除时
    /// 落到同位置（即继任视图），末尾则钳制到新末尾；集合变空时归零。
    /// 剩余 active 视图标记 [`WebView::needs_repaint`]（显示内容可能
    /// 变化，需刷新）。
    pub fn flush_close(&mut self) -> usize {
        let old_active = self.active;
        let mut closed_before = 0usize;
        let mut old_active_closed = false;
        let mut removed = 0usize;
        let mut i = 0usize;
        self.views.retain(|v| {
            let keep = !v.close_scheduled();
            if !keep {
                removed += 1;
                if i < old_active {
                    closed_before += 1;
                } else if i == old_active {
                    old_active_closed = true;
                }
            }
            i += 1;
            keep
        });
        self.active = if self.views.is_empty() {
            0
        } else {
            (old_active - closed_before).min(self.views.len() - 1)
        };
        let _ = old_active_closed; // 两种情况同一公式覆盖：同位置继任或钳制
        if let Some(v) = self.views.get_mut(self.active) {
            v.mark_needs_repaint();
        }
        removed
    }

    /// 切到下一个标签（循环）。
    pub fn select_next(&mut self) {
        if self.views.len() > 1 {
            self.set_active((self.active + 1) % self.views.len());
        }
    }

    /// 切到上一个标签（循环）。
    pub fn select_prev(&mut self) {
        if self.views.len() > 1 {
            self.set_active((self.active + self.views.len() - 1) % self.views.len());
        }
    }

    /// 按索引选择标签（越界忽略，与浏览器 Ctrl+1~9 行为一致）。
    pub fn select(&mut self, index: usize) {
        if index < self.views.len() {
            self.set_active(index);
        }
    }

    fn set_active(&mut self, index: usize) {
        if index != self.active {
            self.active = index;
            self.views[index].mark_needs_repaint();
        }
    }

    /// active 视图索引。
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// 标签数。
    pub fn len(&self) -> usize {
        self.views.len()
    }

    /// 集合是否为空（全部标签已 flush 关闭）。
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }

    /// active 视图（只读）。
    pub fn active(&self) -> &WebView {
        &self.views[self.active]
    }

    /// 按索引取标签（可变；热重载等需要更新非 active 标签的场景）。
    pub fn get_mut(&mut self, index: usize) -> Option<&mut WebView> {
        self.views.get_mut(index)
    }

    /// 全部标签标题（与标签索引对齐；chrome 标签栏绘制用）。
    pub fn titles(&self) -> Vec<&str> {
        self.views.iter().map(|v| v.title.as_str()).collect()
    }

    /// active 视图（可变）。
    pub fn active_mut(&mut self) -> &mut WebView {
        &mut self.views[self.active]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML_A: &str = "<html>A</html>";
    const CSS: &str = "";
    const HTML_B: &str = "<html>B</html>";

    #[test]
    fn new_starts_with_one_active_view() {
        let c = WebViewCollection::new(HTML_A, CSS);
        assert_eq!(c.len(), 1);
        assert_eq!(c.active_index(), 0);
        assert_eq!(c.active().html, HTML_A);
        // 首个视图待渲染（首个 flush 点渲染）。
        assert!(c.active().needs_repaint());
        assert!(!c.active().close_scheduled());
    }

    #[test]
    fn new_tab_activates_new_view() {
        let mut c = WebViewCollection::new(HTML_A, CSS);
        c.new_tab(HTML_B, CSS);
        assert_eq!(c.len(), 2);
        assert_eq!(c.active_index(), 1);
        assert_eq!(c.active().html, HTML_B);
        assert!(c.active().needs_repaint());
    }

    #[test]
    fn close_then_flush_removes_active_and_selects_neighbor() {
        let mut c = WebViewCollection::new(HTML_A, CSS);
        c.new_tab(HTML_B, CSS);
        c.new_tab(HTML_A, CSS); // active = 2
        c.close_active();
        assert!(c.active().close_scheduled());
        // flush 前不移除（延迟更新）。
        assert_eq!(c.len(), 3);
        assert_eq!(c.flush_close(), 1);
        assert_eq!(c.len(), 2);
        // 旧 active（index 2）被移除 → 落到同索引钳制后（index 1）。
        assert_eq!(c.active_index(), 1);
        assert!(c.active().needs_repaint());
    }

    #[test]
    fn flush_close_only_removes_marked_views() {
        let mut c = WebViewCollection::new(HTML_A, CSS);
        c.new_tab(HTML_B, CSS);
        c.select(0);
        c.close_active(); // 关 0
        c.flush_close();
        assert_eq!(c.len(), 1);
        assert_eq!(c.active_index(), 0);
        assert_eq!(c.active().html, HTML_B);
    }

    #[test]
    fn flush_close_all_leaves_empty() {
        let mut c = WebViewCollection::new(HTML_A, CSS);
        c.close_active();
        assert_eq!(c.flush_close(), 1);
        assert!(c.is_empty());
        assert_eq!(c.active_index(), 0);
    }

    #[test]
    fn select_next_prev_wrap_and_mark_dirty() {
        let mut c = WebViewCollection::new(HTML_A, CSS);
        c.new_tab(HTML_B, CSS);
        c.select(0); // 同索引：不标脏（下一断言验证切换才标脏）。
        c.select_next();
        assert_eq!(c.active_index(), 1);
        assert!(c.active().needs_repaint());
        c.select_next(); // 1 → 0（循环）
        assert_eq!(c.active_index(), 0);
        c.select_prev(); // 0 → 1
        assert_eq!(c.active_index(), 1);
    }

    #[test]
    fn select_out_of_range_is_ignored() {
        let mut c = WebViewCollection::new(HTML_A, CSS);
        c.select(5);
        assert_eq!(c.active_index(), 0);
        c.select(1);
        assert_eq!(c.active_index(), 0);
    }

    #[test]
    fn select_same_index_does_not_dirty() {
        let mut c = WebViewCollection::new(HTML_A, CSS);
        c.active_mut().mark_needs_repaint();
        c.active_mut().store_render(vec![0; 4], 1, 1, 1, 1, 1.0);
        assert!(!c.active().needs_repaint());
        c.select(0);
        assert!(!c.active().needs_repaint(), "same index must not dirty");
    }

    #[test]
    fn single_view_select_next_noop() {
        let mut c = WebViewCollection::new(HTML_A, CSS);
        c.select_next();
        c.select_prev();
        assert_eq!(c.active_index(), 0);
    }

    #[test]
    fn get_mut_updates_by_index_without_touching_active() {
        let mut c = WebViewCollection::new(HTML_A, CSS);
        c.new_tab(HTML_B, CSS);
        let v = c.get_mut(0).expect("index 0");
        v.html = String::from("<html>C</html>");
        assert_eq!(c.active_index(), 1, "get_mut must not change active");
        assert_eq!(c.active().html, HTML_B);
        assert!(c.get_mut(5).is_none(), "out of range must be None");
    }

    #[test]
    fn webview_render_state_roundtrip() {
        let mut v = WebView::new(HTML_A, CSS);
        assert!(v.needs_repaint());
        v.store_render(vec![1, 2, 3, 4], 1, 1, 100, 50, 2.0);
        assert!(!v.needs_repaint());
        assert_eq!(v.layout_state(), (100, 50, 2.0));
        let (px, w, h) = v.frame();
        assert_eq!((w, h, px), (1, 1, &[1u8, 2, 3, 4][..]));
        v.mark_needs_repaint();
        assert!(v.needs_repaint());
    }
}
