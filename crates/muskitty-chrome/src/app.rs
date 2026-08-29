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

use muskitty_shell::input::{self, InputEvent, Key, ShortcutAction};
use muskitty_shell::webview::WebViewCollection;

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

/// RGBA8 → softbuffer 0RGB u32（row-major，红在最低字节）。
///
/// 与 shell 后端同款纯函数（shell 退役后由本 crate 持有）。
pub(crate) fn rgba_to_0rgb(data: &[u8]) -> Vec<u32> {
    let mut out = Vec::with_capacity(data.len() / 4);
    for px in data.chunks_exact(4) {
        out.push(((px[2] as u32) << 16) | ((px[1] as u32) << 8) | (px[0] as u32));
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
    modifiers: input::Modifiers,
    /// 最后已知光标位置（物理 px；chrome 命中测试矩形同为物理坐标系。
    /// 页面层逻辑坐标转换待页面命中测试接入时再做）。
    cursor: (f32, f32),
    /// 热重载源（`with_source_file` 模式才有；None = 内嵌 demo，不监视）。
    source: Option<SourceFile>,
    /// 源文件驱动的标签索引（v1 = 启动标签 0；无标签拖拽，仅受关闭影响，
    /// 关闭后越界则停止重载）。
    source_tab: usize,
}

/// Ctrl+T 新标签内容：大号 "Tab N"（可区分，切换可见）。
fn new_tab_content(n: usize) -> String {
    format!(
        "<!doctype html><html><body><h1 style=\"font-size:72px;color:#1a1a1a\">Tab {n}</h1></body></html>"
    )
}

/// 地址栏提交后的占位页（网络未接入，M-1 延后）：回显 URL 作为观测闭环。
fn url_placeholder_page(url: &str) -> String {
    format!(
        "<!doctype html><html><body><h1 style=\"font-size:40px\">{}</h1><p style=\"font-size:16px\">Network not wired yet (M-1 deferred). URL submitted via address bar.</p></body></html>",
        html_escape(url)
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
            modifiers: input::Modifiers::default(),
            cursor: (0.0, 0.0),
            source: None,
            source_tab: 0,
        }
    }

    /// 文件模式：加载自包含 HTML 文件（含 `<style>`）为首标签并**监视
    /// 变更（热重载）**——文件修改后 200ms 内自动重渲染，无需重启。
    pub fn with_source_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let html = std::fs::read_to_string(path)?;
        let mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
        let css = muskitty_shell::page::extract_inline_style(&html);
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
        let css = muskitty_shell::page::extract_inline_style(&html);
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
                muskitty_shell::page::render_page(&a.html, &a.css, vw, vh, scale)
            };
            if let muskitty_renderer::RenderOutput::Pixels {
                width,
                height,
                data,
            } = out
            {
                self.views
                    .active_mut()
                    .store_render(data, width, height, vw, vh, scale);
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
            ChromeEffect::UrlSubmitted(url) if !url.is_empty() => {
                // 网络未接入（M-1 延后）：占位页回显 URL，标签标题
                // 更新为 URL——观测闭环。
                let active = self.views.active_mut();
                active.html = url_placeholder_page(&url);
                active.set_title(&url);
                self.after_tab_change();
            }
            ChromeEffect::UrlSubmitted(_) => {}
        }
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
        if let Some(action) = input::match_shortcut(&ev) {
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

/// winit 逻辑键 → shell [`Key`](input::Key)（含 chrome 地址栏需要的
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
        // 热重载轮询 + 定时唤醒（WaitUntil；无源文件时轮询为 no-op）。
        if self.poll_source() {
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
                self.modifiers = input::Modifiers {
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
        let data = [255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255];
        assert_eq!(rgba_to_0rgb(&data), vec![0x0000FF, 0x00FF00, 0xFF0000]);
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
