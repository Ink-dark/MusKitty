//! B-4 一致性闸门：下游消费的属性必须已在 cascade registry 注册。
//!
//! 背景（2026-09-24 审计）：`border-radius` 四角与
//! `background-repeat/position/size` 在 renderer 侧有完整的消费方
//! （`extract_border_radius` / `extract_background_*`），但 cascade registry 里
//! 没有对应条目——§5 Filtering 对未注册属性**静默丢弃**，于是这些声明根本进
//! 不了 ComputedStyle，绘制侧每次都读到初始值，整条特性是死代码。而 renderer
//! 单测全绿，因为它们是手工构造 `ComputedStyle`，绕开了整条链路。
//!
//! 本测试把"registry 命中"变成可自动判定的契约：扫描 layout/renderer 源码里
//! 读取 ComputedStyle 的属性名字面量，断言每一个都在 registry 中。新增消费方
//! 却忘记注册 → 这里立刻红，而不是静默退化。

use std::path::{Path, PathBuf};

/// 扫描源码中读取 ComputedStyle 的属性名字面量。
///
/// 识别 `…get("<prop>")`（`style.get` / `cs.get` / `cv.get`）与
/// `corner("<prop>")`（`extract_border_radius` 把四个角名传给闭包）两种形式；
/// 只收 ASCII 小写/数字/连字符的串（`--*` 自定义属性另行排除）。
fn property_literals(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for anchor in ["get(\"", "corner(\""] {
        let mut rest = src;
        while let Some(idx) = rest.find(anchor) {
            let after = &rest[idx + anchor.len()..];
            if let Some(end) = after.find('"') {
                let name = &after[..end];
                let is_property = !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
                if is_property {
                    out.push(name.to_string());
                }
            }
            // 前进到锚点之后，继续找下一处（含重叠情形）。
            rest = &rest[idx + anchor.len()..];
        }
    }
    out
}

/// 递归收集目录下所有 `.rs` 文件。
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// 收集 (文件, 属性名) 对。
fn consumed_properties(roots: &[PathBuf]) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for root in roots {
        assert!(
            root.is_dir(),
            "源码目录不存在（独立 crate 未拉取？）：{}",
            root.display()
        );
        rust_files(root, &mut files);
    }
    let mut found = Vec::new();
    for file in files {
        let Ok(src) = std::fs::read_to_string(&file) else {
            continue;
        };
        // 只扫 src（非测试）：测试里的属性名是断言目标，不构成契约。
        let rel = file.to_string_lossy().replace('\\', "/");
        if rel.contains("/tests/") {
            continue;
        }
        let tail = rel.rsplit_once("/src/").map(|(_, t)| t).unwrap_or(&rel);
        for prop in property_literals(&src) {
            found.push((tail.to_string(), prop));
        }
    }
    found.sort();
    found.dedup();
    found
}

#[test]
fn downstream_consumed_properties_are_registered() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let roots = vec![
        manifest.join("src"),
        manifest.join("../muskitty-layout/src"),
    ];
    let found = consumed_properties(&roots);

    // 闸门自检：扫不到东西说明路径/形式变了，不能算通过。
    assert!(
        found.len() >= 40,
        "只扫到 {} 处属性名，扫描疑似失效（应 ≥40）",
        found.len()
    );

    let mut missing: Vec<String> = Vec::new();
    for (file, prop) in &found {
        // 自定义属性（`--*`）不进 registry，由 custom_properties 单独处理。
        if prop.starts_with("--") {
            continue;
        }
        if muskitty_cascade::lookup_property(prop).is_none() {
            missing.push(format!("{file} → {prop}"));
        }
    }
    assert!(
        missing.is_empty(),
        "以下属性被 layout/renderer 读取但未在 cascade registry 注册——声明会在 §5 Filtering \
         被静默丢弃，消费方永远是死代码：\n{}",
        missing.join("\n")
    );
}

#[test]
fn gate_covers_the_properties_that_were_missing_in_the_2026_09_24_audit() {
    // 反向验证闸门有效：本轮修复的 7 个属性必须真的出现在扫描结果里，
    // 否则上面的断言可能因扫描漏掉它们而"空过"。
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let roots = vec![
        manifest.join("src"),
        manifest.join("../muskitty-layout/src"),
    ];
    let owned: Vec<String> = consumed_properties(&roots)
        .into_iter()
        .map(|(_, p)| p)
        .collect();
    for must in [
        "background-repeat",
        "background-position",
        "background-size",
        "border-top-left-radius",
        "border-top-right-radius",
        "border-bottom-right-radius",
        "border-bottom-left-radius",
    ] {
        assert!(
            owned.iter().any(|p| p == must),
            "闸门未覆盖 {must}（扫描形式已变？）"
        );
    }
}
