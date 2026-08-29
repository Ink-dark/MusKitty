//! 输入事件抽象（W-3）。
//!
//! 定义与窗口后端无关的输入事件类型（[`InputEvent`] / [`Key`] /
//! [`Modifiers`] / [`MouseButton`] / [`TouchPhase`]）与 shell 快捷键匹配
//! （[`match_shortcut`]）。坐标一律为**逻辑 px**（物理像素 → 逻辑换算在
//! winit 事件 → [`InputEvent`] 转换时完成，见 `crate::app` 的 `to_logical`）。
//!
//! 本模块只含纯数据与纯函数，**零外部依赖类型**：winit / softbuffer 等
//! 不出现在公共 API（对齐
//! `docs/decisions/2026-08-16-external-dependency-decoupling.md`），
//! 因此 `--no-default-features` 下照常编译。
//!
//! 规划见 `docs/plans/2026-08-23-windowing.md` §W-3。命中测试（事件 →
//! 具体元素）不在本轮，页面层消费由 [`crate::window::PlatformWindow::handle_event`]
//! 承担。

/// 键盘修饰键状态（与后端无关）。
///
/// 从 winit `ModifiersState` 的 `control_key` / `shift_key` / `alt_key` /
/// `super_key` 映射而来（见 `crate::app::modifiers_from_winit`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Ctrl 键。
    pub control: bool,
    /// Shift 键。
    pub shift: bool,
    /// Alt 键。
    pub alt: bool,
    /// Super（Windows / Command）键。
    pub meta: bool,
}

/// 按键（当前最小集）。
///
/// 只含快捷键匹配需要的按键：`Escape`、翻页键（标签前后切换）、
/// 文本输入字符（[`Character`]）。其余 Named 键（Enter / Tab / Home 等）
/// 统一归 [`Other`]——提前穷举违反 Simplicity（当前无消费者）。
///
/// [`Character`]: Key::Character
/// [`Other`]: Key::Other
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Esc 键。
    Escape,
    /// Page Up 键（Ctrl+PageUp → 上一个标签）。
    PageUp,
    /// Page Down 键（Ctrl+PageDown → 下一个标签）。
    PageDown,
    /// Backspace 键（chrome 地址栏删除；不参与快捷键匹配）。
    Backspace,
    /// Enter 键（chrome 地址栏提交；不参与快捷键匹配）。
    Enter,
    /// 文本输入型字符。取 winit `Key::Character` 的首字符；空串归 [`Other`]。
    Character(char),
    /// 已识别但非快捷键相关（Named 非 Escape/PageUp/PageDown / Unidentified / Dead / 空字符）。
    Other,
}

/// 鼠标按键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// 左键。
    Left,
    /// 右键。
    Right,
    /// 中键。
    Middle,
    /// 后退键。
    Back,
    /// 前进键。
    Forward,
    /// 其他按键（winit `Other(u16)` 透传）。
    Other(u16),
}

/// 按键/鼠标按键的按下状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// 按下。
    Pressed,
    /// 释放。
    Released,
}

/// 触摸相位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    /// 触点落下。
    Started,
    /// 触点移动。
    Moved,
    /// 触点抬起。
    Ended,
    /// 触点取消（系统手势打断）。
    Cancelled,
}

/// 窗口输入事件（逻辑 px 坐标）。
///
/// winit 输入事件经 `crate::app` 转换而来；鼠标/滚轮事件的位置为最后
/// 已知光标位置（winit 的 `MouseInput` / `MouseWheel` 本身不带位置）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    /// 按键按下。
    KeyDown {
        /// 按键。
        key: Key,
        /// 当前修饰键状态。
        modifiers: Modifiers,
    },
    /// 按键释放。
    KeyUp {
        /// 按键。
        key: Key,
        /// 当前修饰键状态。
        modifiers: Modifiers,
    },
    /// 鼠标按键按下/释放。
    MouseButton {
        /// 哪个按键。
        button: MouseButton,
        /// 按下或释放。
        state: ButtonState,
        /// 光标位置（逻辑 px）。
        position: (f32, f32),
        /// 当前修饰键状态。
        modifiers: Modifiers,
    },
    /// 鼠标移动。
    MouseMove {
        /// 光标位置（逻辑 px）。
        position: (f32, f32),
        /// 当前修饰键状态。
        modifiers: Modifiers,
    },
    /// 鼠标滚轮 / 触控板滚动。
    MouseWheel {
        /// 水平滚动量。单位混用说明见 `crate::app::wheel_to_input`。
        delta_x: f32,
        /// 垂直滚动量。单位混用说明见 `crate::app::wheel_to_input`。
        delta_y: f32,
        /// 光标位置（逻辑 px）。
        position: (f32, f32),
        /// 当前修饰键状态。
        modifiers: Modifiers,
    },
    /// 触摸事件。
    Touch {
        /// 触摸相位。
        phase: TouchPhase,
        /// 触点位置（逻辑 px）。
        position: (f32, f32),
    },
}

/// Shell 快捷键动作。
///
/// 由 [`match_shortcut`] 匹配产生，在 `crate::app::App::dispatch_input`
/// 中执行（事件分层的第一层：shell 快捷键先于页面层）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    /// 关闭窗口（Esc）。
    Close,
    /// 重新加载页面（Ctrl+R）——重新 parse→layout→render。
    Reload,
    /// 新建标签（Ctrl+T）。
    NewTab,
    /// 关闭当前标签（Ctrl+W）。
    CloseTab,
    /// 切到下一个标签（Ctrl+PageDown）。
    NextTab,
    /// 切到上一个标签（Ctrl+PageUp）。
    PrevTab,
    /// 切到第 `n` 个标签（0-based；Ctrl+1~9 → `TabSelect(0..=8)`）。
    TabSelect(usize),
}

/// Shell 快捷键匹配（纯函数，无窗口依赖）。
///
/// - `KeyDown { Escape, .. }` → [`ShortcutAction::Close`]（任意修饰键组合
///   都关闭，与常见终端/编辑器行为一致）；
/// - `KeyDown { Character('r'|'R'), control && !alt && !meta }` →
///   [`ShortcutAction::Reload`]（允许 Shift，即 Ctrl+Shift+R 也刷新；
///   Alt/Meta 组合不匹配）；
/// - `KeyDown { Character('t'|'T'), control && !alt && !meta }` →
///   [`ShortcutAction::NewTab`]；
/// - `KeyDown { Character('w'|'W'), control && !alt && !meta }` →
///   [`ShortcutAction::CloseTab`]；
/// - `KeyDown { Character('1'..='9'), control && !alt && !meta }` →
///   [`ShortcutAction::TabSelect`]（0-based；Shift+数字产出其他字符，自然不匹配）；
/// - `KeyDown { PageUp/PageDown, control && !alt && !meta }` →
///   [`ShortcutAction::PrevTab`] / [`ShortcutAction::NextTab`]；
/// - 其余 → `None`。
///
/// 合成事件（`is_synthetic`）与按住重复（`repeat`）的过滤由调用方
/// （`crate::app` 的 `window_event`）完成，本函数不做。
pub fn match_shortcut(event: &InputEvent) -> Option<ShortcutAction> {
    let InputEvent::KeyDown { key, modifiers } = event else {
        return None;
    };
    // Ctrl 系快捷键公共条件：Ctrl 按下、Alt/Meta 不按下（Shift 放行，
    // 与 Chrome/常见编辑器一致；Shift+数字产出其他字符自然不匹配）。
    let ctrl = modifiers.control && !modifiers.alt && !modifiers.meta;
    match key {
        Key::Escape => Some(ShortcutAction::Close),
        Key::Character('r' | 'R') if ctrl => Some(ShortcutAction::Reload),
        Key::Character('t' | 'T') if ctrl => Some(ShortcutAction::NewTab),
        Key::Character('w' | 'W') if ctrl => Some(ShortcutAction::CloseTab),
        Key::Character(c @ '1'..='9') if ctrl => {
            Some(ShortcutAction::TabSelect((*c as u8 - b'1') as usize))
        }
        Key::PageUp if ctrl => Some(ShortcutAction::PrevTab),
        Key::PageDown if ctrl => Some(ShortcutAction::NextTab),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_default_is_empty() {
        let m = Modifiers::default();
        assert!(!m.control && !m.shift && !m.alt && !m.meta);
    }

    #[test]
    fn match_shortcut_escape_is_close() {
        let ev = InputEvent::KeyDown {
            key: Key::Escape,
            modifiers: Modifiers::default(),
        };
        assert_eq!(match_shortcut(&ev), Some(ShortcutAction::Close));
    }

    #[test]
    fn match_shortcut_escape_with_modifiers_still_close() {
        let ev = InputEvent::KeyDown {
            key: Key::Escape,
            modifiers: Modifiers {
                control: true,
                alt: true,
                ..Modifiers::default()
            },
        };
        assert_eq!(match_shortcut(&ev), Some(ShortcutAction::Close));
    }

    #[test]
    fn match_shortcut_ctrl_r_is_reload() {
        let ev = InputEvent::KeyDown {
            key: Key::Character('r'),
            modifiers: Modifiers {
                control: true,
                ..Modifiers::default()
            },
        };
        assert_eq!(match_shortcut(&ev), Some(ShortcutAction::Reload));
    }

    #[test]
    fn match_shortcut_ctrl_shift_r_is_reload() {
        let ev = InputEvent::KeyDown {
            key: Key::Character('R'),
            modifiers: Modifiers {
                control: true,
                shift: true,
                ..Modifiers::default()
            },
        };
        assert_eq!(match_shortcut(&ev), Some(ShortcutAction::Reload));
    }

    #[test]
    fn match_shortcut_no_control_r_is_none() {
        let ev = InputEvent::KeyDown {
            key: Key::Character('r'),
            modifiers: Modifiers::default(),
        };
        assert_eq!(match_shortcut(&ev), None);
    }

    #[test]
    fn match_shortcut_alt_ctrl_r_is_none() {
        let ev = InputEvent::KeyDown {
            key: Key::Character('r'),
            modifiers: Modifiers {
                control: true,
                alt: true,
                ..Modifiers::default()
            },
        };
        assert_eq!(match_shortcut(&ev), None);
    }

    #[test]
    fn match_shortcut_ctrl_meta_r_is_none() {
        let ev = InputEvent::KeyDown {
            key: Key::Character('r'),
            modifiers: Modifiers {
                control: true,
                meta: true,
                ..Modifiers::default()
            },
        };
        assert_eq!(match_shortcut(&ev), None);
    }

    #[test]
    fn match_shortcut_character_a_is_none() {
        let ev = InputEvent::KeyDown {
            key: Key::Character('a'),
            modifiers: Modifiers {
                control: true,
                ..Modifiers::default()
            },
        };
        assert_eq!(match_shortcut(&ev), None);
    }

    #[test]
    fn match_shortcut_other_key_is_none() {
        let ev = InputEvent::KeyDown {
            key: Key::Other,
            modifiers: Modifiers::default(),
        };
        assert_eq!(match_shortcut(&ev), None);
    }

    #[test]
    fn match_shortcut_keyup_escape_is_none() {
        // 快捷键只在按下时触发；释放事件不匹配。
        let ev = InputEvent::KeyUp {
            key: Key::Escape,
            modifiers: Modifiers::default(),
        };
        assert_eq!(match_shortcut(&ev), None);
    }

    #[test]
    fn match_shortcut_keyup_ctrl_r_is_none() {
        let ev = InputEvent::KeyUp {
            key: Key::Character('r'),
            modifiers: Modifiers {
                control: true,
                ..Modifiers::default()
            },
        };
        assert_eq!(match_shortcut(&ev), None);
    }

    fn key_down(key: Key, modifiers: Modifiers) -> InputEvent {
        InputEvent::KeyDown { key, modifiers }
    }

    fn ctrl() -> Modifiers {
        Modifiers {
            control: true,
            ..Modifiers::default()
        }
    }

    #[test]
    fn match_shortcut_ctrl_t_is_new_tab() {
        assert_eq!(
            match_shortcut(&key_down(Key::Character('t'), ctrl())),
            Some(ShortcutAction::NewTab)
        );
        assert_eq!(
            match_shortcut(&key_down(Key::Character('T'), ctrl())),
            Some(ShortcutAction::NewTab)
        );
    }

    #[test]
    fn match_shortcut_ctrl_w_is_close_tab() {
        assert_eq!(
            match_shortcut(&key_down(Key::Character('w'), ctrl())),
            Some(ShortcutAction::CloseTab)
        );
    }

    #[test]
    fn match_shortcut_ctrl_digits_select_tab() {
        assert_eq!(
            match_shortcut(&key_down(Key::Character('1'), ctrl())),
            Some(ShortcutAction::TabSelect(0))
        );
        assert_eq!(
            match_shortcut(&key_down(Key::Character('9'), ctrl())),
            Some(ShortcutAction::TabSelect(8))
        );
        // 无 Ctrl：不匹配。
        assert_eq!(
            match_shortcut(&key_down(Key::Character('1'), Modifiers::default())),
            None
        );
    }

    #[test]
    fn match_shortcut_ctrl_page_keys_switch_tabs() {
        assert_eq!(
            match_shortcut(&key_down(Key::PageUp, ctrl())),
            Some(ShortcutAction::PrevTab)
        );
        assert_eq!(
            match_shortcut(&key_down(Key::PageDown, ctrl())),
            Some(ShortcutAction::NextTab)
        );
        // Alt/Meta 组合不匹配。
        assert_eq!(
            match_shortcut(&key_down(
                Key::PageDown,
                Modifiers {
                    control: true,
                    alt: true,
                    ..Modifiers::default()
                }
            )),
            None
        );
    }

    #[test]
    fn match_shortcut_page_keys_without_ctrl_is_none() {
        assert_eq!(
            match_shortcut(&key_down(Key::PageUp, Modifiers::default())),
            None
        );
    }
}
