//! 应用入口：winit 事件循环 + 窗口 + 渲染管线。
//!
//! [`App`] 实现 winit `ApplicationHandler`，管理窗口生命周期：
//! 窗口创建 / 重绘（渲染当前页）/ 尺寸变化重渲染 / 关闭退出。
//! 页面内容通过 [`App::run`] 传入（W-1 硬编码 HTML+CSS，后续接加载器）。
//!
//! 本模块仅在 `winit-backend` feature 下编译（无头场景用
//! [`crate::page::render_page`] 即可，无需事件循环）。

use std::rc::Rc;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{
    ElementState, Modifiers, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{Window, WindowId};

use muskitty_renderer::RenderOutput;

use crate::input::{self, InputEvent, ShortcutAction};
use crate::page::render_page;
use crate::webview::WebViewCollection;
use crate::window::PlatformWindow;
use crate::winit_window::WinitWindow;

/// 初始窗口尺寸（逻辑 px；W-2 起布局视口为逻辑 CSS px，物理分辨率 ×scale）。
pub const DEFAULT_W: u32 = 800;
pub const DEFAULT_H: u32 = 600;

/// 应用状态：窗口 + 多标签 WebView 集合（W-5）。
///
/// 渲染状态（pixels / 布局状态）从 `App` 上移到每份
/// [`crate::webview::WebView`]（每标签一份）；`App` 持有
/// [`WebViewCollection`] 并只作用于 active 标签。`App::run` 传入的
/// 页面内容作为**新建标签的默认内容**。
pub struct App {
    window: Option<Rc<Window>>,
    winit_window: Option<WinitWindow>,
    /// 多标签集合（W-5）；渲染管线只作用于 active 视图。
    views: WebViewCollection,
    /// 当前修饰键状态（由 `ModifiersChanged` 更新；winit 输入事件本身不带）。
    modifiers: input::Modifiers,
    /// 最后已知光标位置（逻辑 px；winit 的 MouseInput/MouseWheel 不带位置）。
    cursor_position: (f32, f32),
}

impl App {
    /// 构造应用状态（尚未创建窗口，`resumed` 时创建）。
    pub fn new(html: &'static str, css: &'static str) -> Self {
        Self {
            window: None,
            winit_window: None,
            views: WebViewCollection::new(html, css),
            modifiers: input::Modifiers::default(),
            cursor_position: (0.0, 0.0),
        }
    }

    /// 以给定页面内容启动窗口应用（阻塞直到窗口关闭）。
    ///
    /// 便捷入口：创建事件循环 + `App` 并运行。
    pub fn run(html: &'static str, css: &'static str) {
        let event_loop = EventLoop::new().expect("event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app = App::new(html, css);
        event_loop.run_app(&mut app).expect("run app");
    }

    /// 以逻辑尺寸 + scale 重新渲染 **active 标签**（W-5：每标签各持
    /// 渲染状态），结果存入该视图（物理分辨率）。
    fn render_at(&mut self, logical_width: u32, logical_height: u32, scale: f32) {
        // 布局用逻辑尺寸（CSS px），栅格化按 scale 输出物理分辨率（W-2）。
        let (html, css) = {
            let view = self.views.active_mut();
            (view.html, view.css)
        };
        let out = render_page(html, css, logical_width, logical_height, scale);
        if let RenderOutput::Pixels {
            width,
            height,
            data,
        } = out
        {
            self.views.active_mut().store_render(
                data,
                width,
                height,
                logical_width,
                logical_height,
                scale,
            );
        }
    }

    /// 提交 active 标签的最近一帧到窗口（无窗口时 no-op）。
    fn present_current(&mut self) {
        if let Some(ww) = &mut self.winit_window {
            let (pixels, width, height) = self.views.active().frame();
            ww.present(pixels, width, height);
        }
    }

    /// 当前 HiDPI scale（无窗口时回退到 active 视图最近渲染的 scale）。
    fn hidpi_scale(&self) -> f32 {
        self.winit_window
            .as_ref()
            .map(|w| w.hidpi_scale_factor())
            .unwrap_or_else(|| self.views.active().layout_state().2)
    }

    /// 强制重新 parse→layout→render（绕过脏检查）并提交——Ctrl+R。
    fn reload(&mut self) {
        let (w, h, scale) = match &self.winit_window {
            Some(ww) => {
                let g = ww.geometry();
                (g.width.max(1), g.height.max(1), ww.hidpi_scale_factor())
            }
            None => return,
        };
        self.render_at(w, h, scale);
        self.present_current();
    }

    /// 事件分层（W-3）：先 shell 快捷键（Esc 关闭 / Ctrl+R 刷新），
    /// 未消费才转页面层（[`PlatformWindow::handle_event`]）。
    fn dispatch_input(&mut self, event_loop: &ActiveEventLoop, event: InputEvent) {
        match input::match_shortcut(&event) {
            Some(ShortcutAction::Close) => event_loop.exit(),
            Some(ShortcutAction::Reload) => self.reload(),
            None => {
                // W-3 无页面命中测试：handle_event 恒返回 false，仅立分发结构。
                if let Some(ww) = &mut self.winit_window {
                    ww.handle_event(event);
                }
            }
        }
    }
}

/// winit 修饰键 → shell [`Modifiers`](input::Modifiers)。
pub(crate) fn modifiers_from_winit(m: &Modifiers) -> input::Modifiers {
    let s = m.state();
    input::Modifiers {
        control: s.control_key(),
        shift: s.shift_key(),
        alt: s.alt_key(),
        meta: s.super_key(),
    }
}

/// winit 逻辑键 → shell [`Key`](input::Key)（当前最小集：Escape / 字符 / Other）。
pub(crate) fn key_from_winit(key: &WinitKey) -> input::Key {
    match key {
        WinitKey::Named(NamedKey::Escape) => input::Key::Escape,
        WinitKey::Character(s) => s
            .chars()
            .next()
            .map(input::Key::Character)
            .unwrap_or(input::Key::Other),
        _ => input::Key::Other,
    }
}

/// winit 按键事件 → shell [`InputEvent`]（Pressed→KeyDown，Released→KeyUp）。
pub(crate) fn keyboard_to_input(
    logical_key: &WinitKey,
    state: ElementState,
    modifiers: input::Modifiers,
) -> InputEvent {
    let key = key_from_winit(logical_key);
    match state {
        ElementState::Pressed => InputEvent::KeyDown { key, modifiers },
        ElementState::Released => InputEvent::KeyUp { key, modifiers },
    }
}

/// winit 鼠标按键 → shell [`MouseButton`](input::MouseButton)。
pub(crate) fn mouse_button_from_winit(button: MouseButton) -> input::MouseButton {
    match button {
        MouseButton::Left => input::MouseButton::Left,
        MouseButton::Right => input::MouseButton::Right,
        MouseButton::Middle => input::MouseButton::Middle,
        MouseButton::Back => input::MouseButton::Back,
        MouseButton::Forward => input::MouseButton::Forward,
        MouseButton::Other(n) => input::MouseButton::Other(n),
    }
}

/// winit 鼠标按键事件 → shell [`InputEvent`]。
pub(crate) fn mouse_button_to_input(
    button: MouseButton,
    state: ElementState,
    position: (f32, f32),
    modifiers: input::Modifiers,
) -> InputEvent {
    InputEvent::MouseButton {
        button: mouse_button_from_winit(button),
        state: match state {
            ElementState::Pressed => input::ButtonState::Pressed,
            ElementState::Released => input::ButtonState::Released,
        },
        position,
        modifiers,
    }
}

/// 物理像素 → 逻辑像素（除以 HiDPI scale）。
///
/// 前置条件 `scale > 0`（来自 `hidpi_scale_factor`，恒 ≥ 1）。
pub(crate) fn to_logical(physical: &PhysicalPosition<f64>, scale: f32) -> (f32, f32) {
    let logical = physical.to_logical::<f64>(scale as f64);
    (logical.x as f32, logical.y as f32)
}

/// winit 滚轮事件 → shell [`InputEvent`]。
///
/// 单位混用（已知缺口）：`LineDelta` 是行、`PixelDelta` 是像素，这里统一
/// 转成 f32 但保留原数值；待滚动消费方出现后再区分单位（本轮无消费者）。
pub(crate) fn wheel_to_input(
    delta: MouseScrollDelta,
    position: (f32, f32),
    modifiers: input::Modifiers,
) -> InputEvent {
    let (delta_x, delta_y) = match delta {
        MouseScrollDelta::LineDelta(x, y) => (x, y),
        MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
    };
    InputEvent::MouseWheel {
        delta_x,
        delta_y,
        position,
        modifiers,
    }
}

/// winit 触摸事件 → shell [`InputEvent`]。
pub(crate) fn touch_to_input(phase: TouchPhase, position: (f32, f32)) -> InputEvent {
    let phase = match phase {
        TouchPhase::Started => input::TouchPhase::Started,
        TouchPhase::Moved => input::TouchPhase::Moved,
        TouchPhase::Ended => input::TouchPhase::Ended,
        TouchPhase::Cancelled => input::TouchPhase::Cancelled,
    };
    InputEvent::Touch { phase, position }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window = Rc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("MusKitty")
                            .with_inner_size(LogicalSize::new(DEFAULT_W as f64, DEFAULT_H as f64)),
                    )
                    .expect("create window"),
            );
            let ww = WinitWindow::new(1, window.clone()).expect("init winit window");
            self.window = Some(window);
            self.winit_window = Some(ww);
            self.winit_window
                .as_ref()
                .expect("just set")
                .request_repaint();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                // 布局用窗口逻辑尺寸 + 当前 scale（W-2）；脏检查含 scale，
                // 任一变化才重渲染，再提交物理分辨率像素。
                let (w, h, scale) = match &self.winit_window {
                    Some(ww) => {
                        let g = ww.geometry();
                        (g.width.max(1), g.height.max(1), ww.hidpi_scale_factor())
                    }
                    None => return,
                };
                if (w, h, scale) != self.views.active().layout_state() {
                    self.render_at(w, h, scale);
                }
                self.present_current();
            }
            WindowEvent::Resized(_) => {
                if let Some(ww) = &self.winit_window {
                    ww.request_repaint();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                // scale 变化（如窗口拖到 HiDPI 显示器）→ 请求重绘，
                // RedrawRequested 按新逻辑尺寸+scale 重新布局渲染。
                if let Some(ww) = &self.winit_window {
                    ww.request_repaint();
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(m) => {
                // winit 输入事件本身不带修饰键，需单独跟踪（W-3）。
                self.modifiers = modifiers_from_winit(&m);
            }
            // 过滤合成事件（焦点切换）与按住重复（避免 Ctrl+R 连发）。
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } if !is_synthetic && !event.repeat => {
                let ev = keyboard_to_input(&event.logical_key, event.state, self.modifiers);
                self.dispatch_input(event_loop, ev);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = to_logical(&position, self.hidpi_scale());
                let ev = InputEvent::MouseMove {
                    position: self.cursor_position,
                    modifiers: self.modifiers,
                };
                self.dispatch_input(event_loop, ev);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let ev = mouse_button_to_input(button, state, self.cursor_position, self.modifiers);
                self.dispatch_input(event_loop, ev);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let ev = wheel_to_input(delta, self.cursor_position, self.modifiers);
                self.dispatch_input(event_loop, ev);
            }
            WindowEvent::Touch(t) => {
                let ev = touch_to_input(t.phase, to_logical(&t.location, self.hidpi_scale()));
                self.dispatch_input(event_loop, ev);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;
    use winit::keyboard::ModifiersState;

    #[test]
    fn modifiers_from_winit_empty() {
        let m = Modifiers::default();
        let out = modifiers_from_winit(&m);
        assert!(!out.control && !out.shift && !out.alt && !out.meta);
    }

    #[test]
    fn modifiers_from_winit_ctrl_shift() {
        let m = Modifiers::from(ModifiersState::CONTROL | ModifiersState::SHIFT);
        let out = modifiers_from_winit(&m);
        assert!(out.control && out.shift && !out.alt && !out.meta);
    }

    #[test]
    fn key_from_winit_escape() {
        let k = WinitKey::Named(NamedKey::Escape);
        assert_eq!(key_from_winit(&k), input::Key::Escape);
    }

    #[test]
    fn key_from_winit_character_first_char() {
        let k = WinitKey::Character("ab".into());
        assert_eq!(key_from_winit(&k), input::Key::Character('a'));
    }

    #[test]
    fn key_from_winit_character_empty() {
        let k = WinitKey::Character("".into());
        assert_eq!(key_from_winit(&k), input::Key::Other);
    }

    #[test]
    fn key_from_winit_named_other() {
        let k = WinitKey::Named(NamedKey::Enter);
        assert_eq!(key_from_winit(&k), input::Key::Other);
    }

    #[test]
    fn keyboard_to_input_pressed_maps_keydown() {
        let k = WinitKey::Character("r".into());
        let m = input::Modifiers::default();
        let ev = keyboard_to_input(&k, ElementState::Pressed, m);
        assert_eq!(
            ev,
            InputEvent::KeyDown {
                key: input::Key::Character('r'),
                modifiers: m,
            }
        );
    }

    #[test]
    fn keyboard_to_input_released_maps_keyup() {
        let k = WinitKey::Named(NamedKey::Escape);
        let m = input::Modifiers::default();
        let ev = keyboard_to_input(&k, ElementState::Released, m);
        assert_eq!(
            ev,
            InputEvent::KeyUp {
                key: input::Key::Escape,
                modifiers: m,
            }
        );
    }

    #[test]
    fn escape_via_keyboard_to_input_matches_shortcut() {
        let k = WinitKey::Named(NamedKey::Escape);
        let ev = keyboard_to_input(&k, ElementState::Pressed, input::Modifiers::default());
        assert_eq!(input::match_shortcut(&ev), Some(ShortcutAction::Close));
    }

    #[test]
    fn ctrl_r_via_keyboard_to_input_matches_shortcut() {
        let k = WinitKey::Character("r".into());
        let m = input::Modifiers {
            control: true,
            ..input::Modifiers::default()
        };
        let ev = keyboard_to_input(&k, ElementState::Pressed, m);
        assert_eq!(input::match_shortcut(&ev), Some(ShortcutAction::Reload));
    }

    #[test]
    fn mouse_button_from_winit_maps_all() {
        assert_eq!(
            mouse_button_from_winit(MouseButton::Left),
            input::MouseButton::Left
        );
        assert_eq!(
            mouse_button_from_winit(MouseButton::Right),
            input::MouseButton::Right
        );
        assert_eq!(
            mouse_button_from_winit(MouseButton::Middle),
            input::MouseButton::Middle
        );
        assert_eq!(
            mouse_button_from_winit(MouseButton::Back),
            input::MouseButton::Back
        );
        assert_eq!(
            mouse_button_from_winit(MouseButton::Forward),
            input::MouseButton::Forward
        );
        assert_eq!(
            mouse_button_from_winit(MouseButton::Other(5)),
            input::MouseButton::Other(5)
        );
    }

    #[test]
    fn mouse_button_to_input_pressed() {
        let m = input::Modifiers {
            control: true,
            ..input::Modifiers::default()
        };
        let ev = mouse_button_to_input(MouseButton::Left, ElementState::Pressed, (10.0, 20.0), m);
        assert_eq!(
            ev,
            InputEvent::MouseButton {
                button: input::MouseButton::Left,
                state: input::ButtonState::Pressed,
                position: (10.0, 20.0),
                modifiers: m,
            }
        );
    }

    #[test]
    fn to_logical_divides_by_scale() {
        let p = PhysicalPosition::new(200.0, 100.0);
        assert_eq!(to_logical(&p, 2.0), (100.0, 50.0));
    }

    #[test]
    fn wheel_to_input_line_delta() {
        let m = input::Modifiers::default();
        let ev = wheel_to_input(MouseScrollDelta::LineDelta(1.0, -2.0), (0.0, 0.0), m);
        assert_eq!(
            ev,
            InputEvent::MouseWheel {
                delta_x: 1.0,
                delta_y: -2.0,
                position: (0.0, 0.0),
                modifiers: m,
            }
        );
    }

    #[test]
    fn wheel_to_input_pixel_delta() {
        let m = input::Modifiers::default();
        let ev = wheel_to_input(
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(120.0, 40.0)),
            (0.0, 0.0),
            m,
        );
        assert_eq!(
            ev,
            InputEvent::MouseWheel {
                delta_x: 120.0,
                delta_y: 40.0,
                position: (0.0, 0.0),
                modifiers: m,
            }
        );
    }

    #[test]
    fn touch_to_input_started() {
        let ev = touch_to_input(TouchPhase::Started, (5.0, 6.0));
        assert_eq!(
            ev,
            InputEvent::Touch {
                phase: input::TouchPhase::Started,
                position: (5.0, 6.0),
            }
        );
    }
}
