//! 浏览器窗口应用：winit 事件循环 + chrome 合成呈现。
//!
//! 职责：创建 OS 窗口（softbuffer 表面）、驱动标签集合（W-5 语义平移：
//! 脏位延迟更新 + 统一 flush）、把输入路由给 chrome（命中测试/地址栏/
//! 快捷键）或页面层、在 `RedrawRequested` 统一合成
//! （[`crate::compositor::compose_frame`]）并提交。
//!
//! 本模块仅在 `winit-backend` feature 下编译；纯函数部分
//! （chrome::model/paint/input、compositor）无窗口可测。

use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{Window, WindowId};

use crate::navigation::{self, NavigationKind, NavigationOutcome};
use crate::shortcut::{self, InputEvent, Key, ShortcutAction};
use crate::webview::WebViewCollection;

use crate::chrome::input::{apply_hover, apply_key, apply_mouse, ChromeEffect, ChromeKey};
use crate::chrome::model::{chrome_height, layout_chrome, ChromeRects, ChromeState};
use crate::chrome::paint::ChromeAssets;
use crate::compositor::compose_frame;

/// 初始窗口尺寸（逻辑 px）。
pub const DEFAULT_W: u32 = 1024;
pub const DEFAULT_H: u32 = 768;

/// 热重载轮询间隔（mtime 轮询；不引 notify 依赖，零 C 依赖约束）。
pub const HOT_RELOAD_POLL: Duration = Duration::from_millis(200);

/// 被监视的源文件（热重载）。
struct SourceFile {
    path: PathBuf,
    mtime: Option<std::time::SystemTime>,
}

/// RGBA8 → softbuffer 0RGB u32（row-major；softbuffer 0.4 契约 =
/// `0x00RRGGBB`，红在位 16-23）。
///
/// 与 shell 后端同款纯函数（shell 退役后由本 crate 持有）。
pub(crate) fn rgba_to_0rgb(data: &[u8]) -> Vec<u32> {
    let mut out = Vec::with_capacity(data.len() / 4);
    for px in data.chunks_exact(4) {
        out.push(((px[0] as u32) << 16) | ((px[1] as u32) << 8) | (px[2] as u32));
    }
    out
}

/// winit 窗口 + softbuffer 表面（chrome 自有窗口后端）。
pub(crate) struct WindowSurface {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
}

impl WindowSurface {
    pub(crate) fn new(window: Rc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let context = softbuffer::Context::new(window.clone())?;
        let surface = softbuffer::Surface::new(&context, window.clone())?;
        Ok(Self { window, surface })
    }

    pub(crate) fn window(&self) -> &Window {
        &self.window
    }

    /// 提交一帧 RGBA8 像素（物理分辨率）。
    pub(crate) fn present(&mut self, data: &[u8], width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface
            .resize(
                std::num::NonZeroU32::new(width).expect("width > 0"),
                std::num::NonZeroU32::new(height).expect("height > 0"),
            )
            .expect("surface resize");
        let pixels = rgba_to_0rgb(data);
        let mut buffer = self.surface.buffer_mut().expect("surface buffer");
        buffer.copy_from_slice(&pixels);
        buffer.present().expect("present frame");
    }
}

/// 浏览器应用状态（窗口 + 标签集合 + chrome UI 状态）。
pub struct App {
    surface: Option<WindowSurface>,
    views: WebViewCollection,
    chrome: ChromeState,
    /// 最近一次布局的 chrome 矩形（命中测试用；resize/flush 时重算）。
    rects: ChromeRects,
    assets: ChromeAssets,
    /// 当前修饰键状态（winit 输入事件本身不带）。
    modifiers: shortcut::Modifiers,
    /// 最后已知光标位置（物理 px；chrome 命中测试矩形同为物理坐标系。
    /// 页面层逻辑坐标转换待页面命中测试接入时再做）。
    cursor: (f32, f32),
    /// 热重载源（`with_source_file` 模式才有；None = 内嵌 demo，不监视）。
    source: Option<SourceFile>,
    /// 源文件驱动的标签索引（v1 = 启动标签 0；无标签拖拽，仅受关闭影响，
    /// 关闭后越界则停止重载）。
    source_tab: usize,
    /// 在途导航结果的接收器（每条地址栏 http(s) 导航一个；结果在
    /// `about_to_wait` 统一吸干应用——网络 IO 在抓取线程完成，事件循环
    /// 只做无阻塞消费）。
    nav_results: Vec<std::sync::mpsc::Receiver<NavigationOutcome>>,
}

/// Ctrl+T 新标签内容：大号 "Tab N"（可区分，切换可见）。
fn new_tab_content(n: usize) -> String {
    format!(
        "<!doctype html><html><body><h1 style=\"font-size:72px;color:#1a1a1a\">Tab {n}</h1></body></html>"
    )
}

/// 不支持 scheme（data:/about:/javascript: 等）的占位页：回显 URL 作为
/// 观测闭环。http/https/file 已由 [`App::navigate_active`] 分流。
fn url_placeholder_page(url: &str) -> String {
    format!(
        "<!doctype html><html><body><h1 style=\"font-size:40px\">{}</h1><p style=\"font-size:16px\">Scheme not supported (http/https/file are). URL submitted via address bar.</p></body></html>",
        navigation::escape_html(url)
    )
}

impl App {
    /// 构造应用（首个标签内容 `html`/`css`）。
    pub fn new(html: &'static str, css: &'static str) -> Self {
        Self {
            surface: None,
            views: WebViewCollection::new(html, css),
            chrome: ChromeState::default(),
            rects: layout_chrome(1, 1, 1.0, 1, &ChromeState::default()),
            assets: ChromeAssets::new(),
            modifiers: shortcut::Modifiers::default(),
            cursor: (0.0, 0.0),
            source: None,
            source_tab: 0,
            nav_results: Vec::new(),
        }
    }

    /// 文件模式：加载自包含 HTML 文件（含 `<style>`）为首标签并**监视
    /// 变更（热重载）**——文件修改后 200ms 内自动重渲染，无需重启。
    pub fn with_source_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let html = std::fs::read_to_string(path)?;
        let mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
        let css = crate::page::extract_inline_style(&html);
        let mut app = Self::new("", "");
        {
            let view = app.views.active_mut();
            view.html = html;
            view.css = css;
            let name = std::path::Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string());
            view.set_title(name);
            view.mark_needs_repaint();
        }
        app.source = Some(SourceFile {
            path: PathBuf::from(path),
            mtime,
        });
        Ok(app)
    }

    /// 启动已构造的应用（阻塞直到窗口关闭）。
    pub fn start(mut self) {
        let event_loop = EventLoop::new().expect("event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        event_loop.run_app(&mut self).expect("run app");
    }

    /// 轮询源文件：mtime 变化 → 重新读入内容到源标签并标脏。返回是否变化。
    fn poll_source(&mut self) -> bool {
        let Some(path) = self.source.as_ref().map(|s| s.path.clone()) else {
            return false;
        };
        let mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
        if mtime == self.source.as_ref().and_then(|s| s.mtime) {
            return false;
        }
        // 文件读失败（编辑器原子写瞬间）保留旧内容，下轮再试。
        let Ok(html) = std::fs::read_to_string(&path) else {
            return false;
        };
        if let Some(src) = self.source.as_mut() {
            src.mtime = mtime;
        }
        let css = crate::page::extract_inline_style(&html);
        let title = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        let Some(view) = self.views.get_mut(self.source_tab) else {
            return false;
        };
        let changed = view.html != html;
        view.html = html;
        view.css = css;
        if let Some(t) = title {
            view.set_title(t);
        }
        view.mark_needs_repaint();
        changed
    }

    /// 启动浏览器窗口（阻塞直到关闭）。
    pub fn run(html: &'static str, css: &'static str) {
        let event_loop = EventLoop::new().expect("event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app = App::new(html, css);
        event_loop.run_app(&mut app).expect("run app");
    }

    /// 当前窗口几何 `(物理宽, 物理高, scale)`。
    fn phys_geometry(&self) -> (u32, u32, f32) {
        match &self.surface {
            Some(s) => {
                let size = s.window().inner_size();
                let scale = s.window().scale_factor() as f32;
                (size.width.max(1), size.height.max(1), scale)
            }
            None => (1, 1, 1.0),
        }
    }

    /// 页面视口的逻辑尺寸（窗口逻辑尺寸减 chrome 高度），≥ 1×1。
    fn viewport_logical(&self) -> (u32, u32, f32) {
        let (pw, ph, scale) = self.phys_geometry();
        let logical_w = ((pw as f32) / scale).max(1.0) as u32;
        let logical_h = ((ph as f32 - chrome_height(scale)) / scale).max(1.0) as u32;
        (logical_w, logical_h, scale)
    }

    /// 统一 flush 点（`RedrawRequested`）：关标签延迟移除（空则退出）、
    /// active 脏位/视口变化才重渲染（布局视口 = 页面视口逻辑尺寸）、
    /// chrome 布局 + 合成 + 提交。
    fn flush(&mut self, event_loop: &ActiveEventLoop) {
        self.views.flush_close();
        if self.views.is_empty() {
            event_loop.exit();
            return;
        }

        let (pw, ph, scale) = self.phys_geometry();
        let (vw, vh, _) = self.viewport_logical();

        // active 页面：脏或视口变化才重渲染。
        let dirty = {
            let a = self.views.active_mut();
            a.needs_repaint() || (vw, vh, scale) != a.layout_state()
        };
        if dirty {
            let out = {
                let a = self.views.active();
                crate::page::render_page(&a.html, &a.css, vw, vh, scale)
            };
            match out {
                Ok(muskitty_renderer::RenderOutput::Pixels {
                    width,
                    height,
                    data,
                }) => {
                    self.views
                        .active_mut()
                        .store_render(data, width, height, vw, vh, scale);
                }
                Ok(_) => {}
                Err(e) => {
                    // F-13（审计 S-7）：layout 失败降级——保留上一帧并把
                    // 本次视口记为已渲染（更新 layout_state），避免每个
                    // flush 点重复失败刷屏；绝不 abort 浏览器进程。
                    eprintln!("muskitty-chrome: page render failed: {e}");
                    let last = {
                        let (px, w, h) = self.views.active().frame();
                        (px.to_vec(), w, h)
                    };
                    self.views
                        .active_mut()
                        .store_render(last.0, last.1, last.2, vw, vh, scale);
                }
            }
        }

        // chrome 布局（物理矩形）+ 合成 + 提交。
        self.rects = layout_chrome(pw, ph, scale, self.views.len(), &self.chrome);
        let titles = self.views.titles();
        let (page_data, page_w, page_h) = self.views.active().frame();
        if let Some(frame) = compose_frame(
            pw,
            ph,
            (page_data, page_w, page_h),
            self.rects.page_viewport,
            &self.rects,
            &self.chrome,
            &titles,
            self.views.active_index(),
            &mut self.assets,
        ) {
            if let Some(surface) = &mut self.surface {
                surface.present(frame.data(), frame.width(), frame.height());
            }
        }
    }

    /// 请求一次 flush（重绘）。
    fn request_flush(&mut self) {
        if let Some(surface) = &self.surface {
            surface.window().request_redraw();
        }
    }

    /// 标签集合动作的统一收尾（标脏 + 请求重绘）。
    fn after_tab_change(&mut self) {
        self.views.active_mut().mark_needs_repaint();
        self.request_flush();
    }

    /// 执行 chrome 效果（命中测试/键盘产出）。
    fn run_effect(&mut self, effect: ChromeEffect) {
        match effect {
            ChromeEffect::Repaint => self.request_flush(),
            ChromeEffect::SwitchTab(i) => {
                self.views.select(i);
                self.after_tab_change();
            }
            ChromeEffect::CloseTab(i) => {
                // 点击非活动标签的 ×：先选中再关闭（close_active 语义）。
                self.views.select(i);
                self.views.close_active();
                self.after_tab_change();
            }
            ChromeEffect::NewTab => {
                let n = self.views.len() + 1;
                self.views.new_tab(new_tab_content(n), String::new());
                self.views.active_mut().set_title(format!("Tab {n}"));
                self.after_tab_change();
            }
            ChromeEffect::ReloadPage => self.after_tab_change(),
            ChromeEffect::UrlSubmitted(url) if !url.is_empty() => self.navigate_active(&url),
            ChromeEffect::UrlSubmitted(_) => {}
        }
    }

    /// 地址栏提交 → 导航（Phase 5 接驳，HTML Standard §7.2 极简子集：
    /// 顶级文档 GET）。
    ///
    /// 分类见 [`navigation::classify_url`]。加载期间保留旧页，标题先更新
    /// 为输入（观测闭环），到站后由 [`Self::drain_navigation_results`]
    /// 回填。file 加载同步完成；http(s) 在独立线程抓取。所有分支推进
    /// 导航代数——同标签先发的在途结果因此全部失效。
    fn navigate_active(&mut self, input: &str) {
        match navigation::classify_url(input) {
            NavigationKind::Http(url) => {
                let tab = self.views.active_index();
                let epoch = {
                    let view = self.views.active_mut();
                    view.navigation_epoch += 1;
                    view.navigation_epoch
                };
                self.views.active_mut().set_title(&url);
                self.nav_results
                    .push(navigation::spawn_http_navigation(url, tab, epoch));
                self.after_tab_change();
            }
            NavigationKind::File(path) => {
                {
                    let view = self.views.active_mut();
                    view.navigation_epoch += 1;
                    match std::fs::read_to_string(&path) {
                        Ok(html) => {
                            view.css = crate::page::extract_inline_style(&html);
                            view.html = html;
                            let name = std::path::Path::new(&path)
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| path.clone());
                            view.set_title(name);
                        }
                        Err(e) => {
                            view.css = String::new();
                            view.html = navigation::error_page(&path, &e.to_string());
                            view.set_title(&path);
                        }
                    }
                }
                self.after_tab_change();
            }
            NavigationKind::Unsupported(url) => {
                {
                    let view = self.views.active_mut();
                    view.navigation_epoch += 1;
                    view.css = String::new();
                    view.html = url_placeholder_page(&url);
                    view.set_title(&url);
                }
                self.after_tab_change();
            }
        }
    }

    /// 吸干在途导航结果并应用到目标标签。返回是否有页面变化。
    ///
    /// 结果携带提交时的 `(tab, epoch)`：标签不存在（已关）或代数不匹配
    /// （改址后的过期导航）静默丢弃；channel 断开（线程已发完退出）→
    /// 移除接收器。
    fn drain_navigation_results(&mut self) -> bool {
        let mut outcomes = Vec::new();
        self.nav_results.retain_mut(|rx| loop {
            match rx.try_recv() {
                Ok(outcome) => outcomes.push(outcome),
                Err(std::sync::mpsc::TryRecvError::Empty) => break true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break false,
            }
        });
        let mut changed = false;
        for outcome in outcomes {
            let Some(view) = self.views.get_mut(outcome.tab) else {
                continue;
            };
            if view.navigation_epoch != outcome.epoch {
                continue;
            }
            match outcome.result {
                Ok(doc) => {
                    view.html = doc.html;
                    view.css = doc.css;
                    view.set_title(doc.final_url);
                }
                Err(message) => {
                    view.css = String::new();
                    view.html = navigation::error_page(&outcome.url, &message);
                    view.set_title(&outcome.url);
                }
            }
            view.mark_needs_repaint();
            changed = true;
        }
        changed
    }

    /// 键盘分发：地址栏 Esc 取焦点 > shell 快捷键（Ctrl+T/W/1~9/
    /// PageUp/Down、Ctrl+R 刷新、Esc 关窗）> 地址栏输入。
    fn dispatch_key(&mut self, event_loop: &ActiveEventLoop, key: Key) {
        // 地址栏聚焦时 Esc 先取消焦点（不关窗）。
        if self.chrome.address_focused && key == Key::Escape {
            let effect = apply_key(&mut self.chrome, ChromeKey::Escape);
            self.run_effect(effect);
            return;
        }
        let ev = InputEvent::KeyDown {
            key,
            modifiers: self.modifiers,
        };
        if let Some(action) = shortcut::match_shortcut(&ev) {
            match action {
                ShortcutAction::Close => event_loop.exit(),
                ShortcutAction::Reload => self.after_tab_change(),
                ShortcutAction::NewTab => self.run_effect(ChromeEffect::NewTab),
                ShortcutAction::CloseTab => {
                    self.views.close_active();
                    self.after_tab_change();
                }
                ShortcutAction::NextTab => {
                    self.views.select_next();
                    self.after_tab_change();
                }
                ShortcutAction::PrevTab => {
                    self.views.select_prev();
                    self.after_tab_change();
                }
                ShortcutAction::TabSelect(n) => {
                    self.views.select(n);
                    self.after_tab_change();
                }
                ShortcutAction::FocusAddress => {
                    self.chrome.address_focused = true;
                    self.request_flush();
                }
            }
            return;
        }
        // 地址栏输入（无 Ctrl/Alt/Meta 的字符才进地址栏）。
        let plain = !self.modifiers.control && !self.modifiers.alt && !self.modifiers.meta;
        if self.chrome.address_focused && plain {
            let ck = match key {
                Key::Character(c) => Some(ChromeKey::Char(c)),
                Key::Backspace => Some(ChromeKey::Backspace),
                Key::Enter => Some(ChromeKey::Enter),
                _ => None,
            };
            if let Some(ck) = ck {
                let effect = apply_key(&mut self.chrome, ck);
                self.run_effect(effect);
            }
        }
    }
}

/// winit 逻辑键 → shell [`Key`](shortcut::Key)（含 chrome 地址栏需要的
/// Backspace / Enter；两者不参与快捷键匹配）。
fn key_from_winit(key: &WinitKey) -> Key {
    match key {
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Named(NamedKey::PageUp) => Key::PageUp,
        WinitKey::Named(NamedKey::PageDown) => Key::PageDown,
        WinitKey::Named(NamedKey::Backspace) => Key::Backspace,
        WinitKey::Named(NamedKey::Enter) => Key::Enter,
        WinitKey::Character(s) => s.chars().next().map(Key::Character).unwrap_or(Key::Other),
        _ => Key::Other,
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_none() {
            let window = Rc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("MusKitty Browser")
                            .with_inner_size(LogicalSize::new(DEFAULT_W as f64, DEFAULT_H as f64)),
                    )
                    .expect("create window"),
            );
            let surface = WindowSurface::new(window).expect("init surface");
            surface.window().request_redraw();
            self.surface = Some(surface);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // 在途导航结果 + 热重载轮询（WaitUntil 定时唤醒；无源文件时轮询
        // 为 no-op）。有变化 → 请求重绘（RedrawRequested 统一 flush）。
        let nav_changed = self.drain_navigation_results();
        if nav_changed || self.poll_source() {
            self.request_flush();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + HOT_RELOAD_POLL));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => self.flush(event_loop),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.request_flush();
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.modifiers = shortcut::Modifiers {
                    control: s.control_key(),
                    shift: s.shift_key(),
                    alt: s.alt_key(),
                    meta: s.super_key(),
                };
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                if apply_hover(&mut self.chrome, &self.rects, self.cursor.0, self.cursor.1) {
                    self.request_flush();
                }
            }
            WindowEvent::MouseInput { state, button, .. }
                if state == ElementState::Pressed && button == MouseButton::Left =>
            {
                let (x, y) = self.cursor;
                let effect = apply_mouse(&mut self.chrome, &self.rects, x, y);
                self.run_effect(effect);
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } if !is_synthetic && !event.repeat && event.state == ElementState::Pressed => {
                // 只吃按下：不过滤 Released 会让每次按键进两次字符
                // （退格也删两次）。
                let key = key_from_winit(&event.logical_key);
                self.dispatch_key(event_loop, key);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_to_0rgb_byte_order() {
        // softbuffer 0.4 契约（lib.rs "Data representation"）：u32 = 0x00RRGGBB，
        // 红在位 16-23。曾把 B 放进高位导致窗口整帧 R/B 互换（实测 demo 页
        // 蓝 #2196f3 显示为橙）。
        let data = [255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255];
        assert_eq!(rgba_to_0rgb(&data), vec![0xFF0000, 0x00FF00, 0x0000FF]);
    }

    #[test]
    fn new_tab_content_shows_number() {
        let html = new_tab_content(3);
        assert!(html.contains("Tab 3"));
    }

    #[test]
    fn url_placeholder_escapes_html() {
        let page = url_placeholder_page("<script>x</script>");
        assert!(!page.contains("<script>"));
        assert!(page.contains("&lt;script&gt;"));
    }

    #[test]
    fn navigation_result_applies_to_tab_and_stale_dropped() {
        use crate::navigation::NavigationDoc;
        let mut app = App::new("<html>old</html>", "");
        // 模拟已提交导航：标签代数推进到 1。
        app.views.active_mut().navigation_epoch = 1;
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(NavigationOutcome {
            tab: 0,
            epoch: 1,
            url: "https://example.com".to_string(),
            result: Ok(NavigationDoc {
                final_url: "https://example.com/".to_string(),
                html: "<html>new</html>".to_string(),
                css: "p{color:red}".to_string(),
            }),
        })
        .unwrap();
        drop(tx);
        app.nav_results.push(rx);

        // 到站：应用 + 重绘；断开的接收器被清理。
        assert!(app.drain_navigation_results());
        assert_eq!(app.views.active().html, "<html>new</html>");
        assert_eq!(app.views.active().css, "p{color:red}");
        assert_eq!(app.views.active().title, "https://example.com/");
        assert!(app.views.active().needs_repaint());
        assert!(app.nav_results.is_empty());
        assert!(!app.drain_navigation_results(), "no pending results");

        // 代数不匹配（改址后的过期导航）：不应用，接收器照样清理。
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(NavigationOutcome {
            tab: 0,
            epoch: 99,
            url: "https://stale.example".to_string(),
            result: Ok(NavigationDoc {
                final_url: "https://stale.example/".to_string(),
                html: "<html>stale</html>".to_string(),
                css: String::new(),
            }),
        })
        .unwrap();
        drop(tx);
        app.nav_results.push(rx);
        assert!(!app.drain_navigation_results());
        assert_eq!(app.views.active().html, "<html>new</html>");
        assert!(app.nav_results.is_empty());
    }

    #[test]
    fn navigation_error_result_renders_error_page() {
        let mut app = App::new("<html>old</html>", "");
        app.views.active_mut().navigation_epoch = 3;
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(NavigationOutcome {
            tab: 0,
            epoch: 3,
            url: "https://down.example".to_string(),
            result: Err("connection refused".to_string()),
        })
        .unwrap();
        drop(tx);
        app.nav_results.push(rx);

        assert!(app.drain_navigation_results());
        assert!(app.views.active().html.contains("Navigation failed"));
        assert!(app.views.active().html.contains("connection refused"));
        assert!(app.views.active().css.is_empty());
        assert_eq!(app.views.active().title, "https://down.example");
    }

    #[test]
    fn navigate_active_file_scheme_loads_local_html() {
        let dir = std::env::temp_dir();
        let path = dir.join("muskitty_nav_file_test.html");
        std::fs::write(
            &path,
            "<!doctype html><html><head><style>p{color:blue}</style></head><body><p>nav-file</p></body></html>",
        )
        .unwrap();
        let mut app = App::new("<html>old</html>", "");
        app.navigate_active(&format!("file:///{}", path.to_string_lossy()));
        assert!(app.views.active().html.contains("nav-file"));
        assert_eq!(app.views.active().css, "p{color:blue}\n");
        assert_eq!(app.views.active().title, "muskitty_nav_file_test.html");
        assert!(app.views.active().needs_repaint());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn navigate_active_unsupported_shows_placeholder() {
        let mut app = App::new("<html>old</html>", "");
        app.navigate_active("data:text/html,<b>hi</b>");
        assert!(app.views.active().html.contains("Scheme not supported"));
        assert_eq!(app.views.active().title, "data:text/html,<b>hi</b>");
    }

    #[test]
    fn navigate_active_http_queues_result_and_bumps_epoch() {
        let mut app = App::new("<html>old</html>", "");
        // 必然快速连接失败的地址（同 network crate 策略）：只验证入队与
        // 代数推进，不等结果（结果应用已有专门用例）。
        app.navigate_active("https://127.0.0.1:1/");
        assert_eq!(app.views.active().navigation_epoch, 1);
        assert_eq!(app.views.active().title, "https://127.0.0.1:1/");
        assert_eq!(app.nav_results.len(), 1);
        assert!(app.views.active().needs_repaint());
    }

    #[test]
    fn hot_reload_picks_up_file_change() {
        let dir = std::env::temp_dir();
        let path = dir.join("muskitty_hot_reload_test.html");
        std::fs::write(
            &path,
            "<!doctype html><html><head><style>div{width:10px;height:10px}</style></head><body><div></div></body></html>",
        )
        .unwrap();
        let path_str = path.to_string_lossy().into_owned();
        let mut app = App::with_source_file(&path_str).expect("load file");
        // 首标签内容/标题已加载。
        assert!(app.views.active().html.contains("width:10px"));
        assert_eq!(app.views.active().title, "muskitty_hot_reload_test.html");
        assert!(app.source.is_some());
        // 未变更：轮询返回 false。
        assert!(!app.poll_source());
        // 修改文件 → 轮询返回 true，内容更新 + 标脏。
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(
            &path,
            "<!doctype html><html><head><style>div{width:99px}</style></head><body><div></div></body></html>",
        )
        .unwrap();
        assert!(app.poll_source());
        assert!(app.views.active().html.contains("width:99px"));
        assert!(app.views.active().needs_repaint());
        // 无变更再轮询：false。
        assert!(!app.poll_source());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn poll_source_without_source_is_noop() {
        let mut app = App::new("<html></html>", "");
        assert!(app.source.is_none());
        assert!(!app.poll_source());
    }
}
