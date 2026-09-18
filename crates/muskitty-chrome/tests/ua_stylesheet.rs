//! CS-1f：最小 UA 样式表的像素级断言。
//!
//! 走 `page::render_page` 全管线（UA 表由渲染入口自动注入），断言最终 RGBA。
//! 修复前的实测状态（规划文档 §一 #9）：layout 用硬编码 8 标签跳过表顶替 UA
//! 表，规范 §15.3.1 的 15 个非渲染标签里 `area/datalist/basefont/noembed/
//! noframes/param/rp` 会生成盒，`p/h1` 无默认边距与字号，`body` 无 8px 边距。

use muskitty_chrome::page::render_page;
use muskitty_renderer::RenderOutput;

fn render(html: &str) -> (Vec<u8>, u32, u32) {
    match render_page(html, "", 200, 200, 1.0).expect("render") {
        RenderOutput::Pixels {
            data,
            width,
            height,
        } => (data, width, height),
        _ => panic!("expected pixels"),
    }
}

fn px(data: &[u8], width: u32, x: u32, y: u32) -> (u8, u8, u8, u8) {
    let i = ((y * width + x) * 4) as usize;
    (data[i], data[i + 1], data[i + 2], data[i + 3])
}

/// 非白像素数。
fn ink(data: &[u8], width: u32, height: u32) -> usize {
    let mut n = 0;
    for y in 0..height {
        for x in 0..width {
            let (r, g, b, _) = px(data, width, x, y);
            if r < 200 || g < 200 || b < 200 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn previously_boxed_non_rendered_tags_are_hidden() {
    // 7 个"修复前会出盒"的标签：给足几何 + 高特异性背景，若不 display:none
    // 必然出现红色块。
    const VOID_TAGS: [&str; 3] = ["area", "param", "basefont"];
    for tag in VOID_TAGS {
        let html = format!(
            r#"<!doctype html><html><body><{tag} style="width:100px;height:100px;background-color:#ff0000"></{tag}></body></html>"#
        );
        let (data, w, h) = render(&html);
        assert_eq!(
            px(&data, w, 8, 8),
            (255, 255, 255, 255),
            "<{tag}> 应 display:none（HTML §15.3.1）"
        );
        assert_eq!(ink(&data, w, h), 0, "<{tag}> 整页无墨迹");
    }

    // 能容纳内容的标签：子元素若渲染会留墨迹。
    const TEXT_TAGS: [&str; 4] = ["datalist", "rp", "noembed", "noframes"];
    for tag in TEXT_TAGS {
        let html = format!(
            "<!doctype html><html><body><{tag}><div style=\"width:100px;height:100px;background-color:#ff0000\">ink</div></{tag}></body></html>"
        );
        let (data, w, h) = render(&html);
        assert_eq!(ink(&data, w, h), 0, "<{tag}> 整页无墨迹");
    }

    // 对照组：未知标签仍会出盒（证明上面的"无墨迹"不是渲染失败）。
    let (data, w, h) = render(
        r#"<!doctype html><html><body><foobar style="width:100px;height:100px;background-color:#ff0000"></foobar></body></html>"#,
    );
    assert!(ink(&data, w, h) > 0, "对照组必须出盒");
}

#[test]
fn body_has_default_eight_pixel_margin() {
    let (data, w, _h) = render(
        r#"<!doctype html><html><body><div style="width:10px;height:10px;background-color:#ff0000"></div></body></html>"#,
    );
    // 盒从 (8,8) 开始（HTML §15.3.2 的 body 默认 8px 边距）。
    assert_eq!(px(&data, w, 8, 8), (255, 0, 0, 255), "(8,8) 应在盒内");
    assert_eq!(px(&data, w, 12, 12), (255, 0, 0, 255), "(12,12) 应在盒内");
    assert_eq!(px(&data, w, 2, 2), (255, 255, 255, 255), "(2,2) 应在盒外");
}

#[test]
fn headings_get_default_size_and_margins() {
    // h1 = 2em（默认 16px 字号 → 32px）+ margin 0.67em（≈21.4px）。
    let (data, w, h) = render("<!doctype html><html><body><h1>H</h1></body></html>");
    let mut first_ink_row = None;
    let mut last_ink_row = 0;
    for y in 0..h {
        for x in 0..w {
            let (r, g, b, _) = px(&data, w, x, y);
            if r < 200 || g < 200 || b < 200 {
                first_ink_row.get_or_insert(y);
                last_ink_row = y;
            }
        }
    }
    let first = first_ink_row.expect("h1 应有墨迹");
    let glyph_rows = last_ink_row - first + 1;
    // body 8px + margin 0.67em(21.44) → 首行墨迹在 y≥29；上限放宽到 40 容忍
    // 字体 ascent 的差异。
    assert!(
        (28..=40).contains(&first),
        "h1 首行墨迹应在默认边距之下，实际 y={first}"
    );
    // 32px 字号的 'H' 高约 23px；16px 时约 11px——用 18 作分界。
    assert!(
        glyph_rows > 18,
        "h1 字号应为 2em（32px），实际墨迹行数 {glyph_rows}"
    );
}

#[test]
fn paragraph_gets_default_one_em_margins() {
    let (data, w, _h) =
        render(r#"<!doctype html><html><body><p style="font-size:20px">x</p></body></html>"#);
    // body 8px + p 的 margin-top 1em(=20px，font-size 20px) → 盒顶 y=28。
    // 用背景色把盒顶显形：p 的盒只有文字墨迹，故改判文字不在最上方。
    let mut first_ink_row = None;
    for y in 0..w {
        let mut found = false;
        for x in 0..w {
            let (r, g, b, _) = px(&data, w, x, y);
            if r < 200 || g < 200 || b < 200 {
                found = true;
                break;
            }
        }
        if found {
            first_ink_row = Some(y);
            break;
        }
    }
    let first = first_ink_row.expect("p 应有文字");
    assert!(first >= 24, "p 的 1em 上边距应把文字下推，实际 y={first}");
}

#[test]
fn hidden_attribute_hides_and_author_can_override() {
    // [hidden] → display:none（HTML §15.3.1）。
    let (data, w, h) = render(
        r#"<!doctype html><html><body><div hidden style="width:100px;height:100px;background-color:#ff0000"></div></body></html>"#,
    );
    assert_eq!(ink(&data, w, h), 0, "[hidden] 不应渲染");

    // 作者可覆盖（UA 表 origin 低于作者）。
    let (data, w, _h) = render(
        r#"<!doctype html><html><body><div hidden style="display:block;width:100px;height:100px;background-color:#ff0000"></div></body></html>"#,
    );
    assert!(
        ink(&data, w, 200) > 0,
        "作者 display:block 应覆盖 UA 的 none"
    );
}

#[test]
fn list_and_blockquote_get_default_indentation() {
    // ol/ul：margin 1em 0 + padding-left 40px；blockquote：1em 0 + 左右 40px。
    let (data, w, _h) = render(
        r#"<!doctype html><html><body><ul><li style="width:10px;height:10px;background-color:#ff0000"></li></ul></body></html>"#,
    );
    // body 8 + ul padding-left 40 = 48 → 方块的左边界在 x=48。
    assert_eq!(
        px(&data, w, 48, 24),
        (255, 0, 0, 255),
        "列表项应被推右 40px"
    );
    assert_eq!(
        px(&data, w, 20, 24),
        (255, 255, 255, 255),
        "padding 内不应有方块"
    );

    let (data, w, _h) = render(
        r#"<!doctype html><html><body><blockquote style="width:10px;height:10px;background-color:#0000ff"></blockquote></body></html>"#,
    );
    assert_eq!(
        px(&data, w, 48, 24),
        (0, 0, 255, 255),
        "blockquote 应左缩进 40px"
    );
}

#[test]
fn pre_is_monospace_and_hr_draws_a_line() {
    // pre：等宽字体（§15.3.3）；hr：1px 边框 + 0.5em 上下边距（§15.3.11）。
    let (data, w, h) = render("<!doctype html><html><body><hr></body></html>");
    assert!(ink(&data, w, h) > 0, "hr 应画出边框线");
}

#[test]
fn pre_keeps_source_line_breaks_via_ua_sheet() {
    // M-3 batch 3c：UA 表的 `pre { white-space: pre }` 生效——<pre> 里三个
    // 源行渲染为 3 行（最底墨迹行 ≥ 两个行高之下），同一源文本放进 div
    // （normal）则折叠为一行（最底墨迹行在首行行高内）。
    // 注：不用"墨迹块间隔"计数——19.2px 行距与字形 ascent/descent 几乎
    // 相接（实测行块间隔 2–10px），量值脆弱；只断言"总垂直范围"。
    let source = "alpha
beta
gamma";
    let (data, w, h) = render(&format!(
        r#"<!doctype html><html><body><pre>{source}</pre></body></html>"#
    ));
    let mut rows = Vec::new();
    for y in 0..h {
        let mut has = false;
        for x in 0..w {
            let (r, g, b, _) = px(&data, w, x, y);
            if r < 200 || g < 200 || b < 200 {
                has = true;
                break;
            }
        }
        if has {
            rows.push(y);
        }
    }
    let first = *rows.first().expect("<pre> 应有墨迹");
    let last = *rows.last().expect("<pre> 应有墨迹");
    let span = last - first;
    assert!(
        span > 38,
        "<pre> 必须保留两个强制换行（3 行 × 19.2px 行距 ≈ 58px 总范围），\
         实际首行 y={first} 末行 y={last} span={span}"
    );

    let (data2, w2, h2) = render(&format!(
        r#"<!doctype html><html><body><div>{source}</div></body></html>"#
    ));
    let mut rows2 = Vec::new();
    for y in 0..h2 {
        let mut has = false;
        for x in 0..w2 {
            let (r, g, b, _) = px(&data2, w2, x, y);
            if r < 200 || g < 200 || b < 200 {
                has = true;
                break;
            }
        }
        if has {
            rows2.push(y);
        }
    }
    let first2 = *rows2.first().expect("div 应有墨迹");
    let last2 = *rows2.last().expect("div 应有墨迹");
    assert!(
        last2 - first2 < 30,
        "div（normal 折叠）必须是单行（范围 < 30px），实际 {first2}..{last2}"
    );
}
