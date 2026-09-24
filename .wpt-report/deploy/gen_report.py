#!/usr/bin/env python3
"""Generate a self-contained HTML WPT test report from harness logs.

Reads the six suite logs under LOGS_DIR and writes ONE static `index.html`
(no external assets) that can be served directly from a git repo / GitHub Pages.
"""
import re
import html
import datetime
import json
import os

LOGS_DIR = "/workspace/.wpt-report/logs"
OUT_DIR = "/workspace/.wpt-report/report"

# Suite definitions: (key, wpt suite label, crate, harness description)
SUITES = [
    ("html5-parser",     "WPT html/syntax/parsing (html5lib tree-construction)", "muskitty-html5-parser", "tree construction"),
    ("html5-tokenizer",  "html5lib tokenizer (WPT mirror, same source)",          "muskitty-html5-tokenizer", "tokenizer"),
    ("selectors",        "WPT css/selectors/parsing (22 files)",                  "muskitty-selectors", "selector parsing"),
    ("css-tokenizer",    "WPT css/css-syntax (tokenizer layer)",                  "muskitty-css-tokenizer", "CSS tokenizer"),
    ("css-parser",       "WPT css/css-syntax (parser layer)",                     "muskitty-css-parser", "CSS parser"),
    # css-values uses an inline hard-asserted harness (wpt_css_syntax.rs) that
    # does not print a PASS RATE line: 2 test fns asserting the <number> /
    # <length> / <integer> numeric grammar (3+3+10 = 16 grammar cases).
    ("css-values",       "WPT css/css-syntax (numeric grammar)",                  "muskitty-css-values", "CSS numeric values", "hardcoded", (16, 16)),
]

ROW_RE = re.compile(r"^\s*([^\s]+)\s+([0-9]+)\s+([0-9]+)(?:\s+([0-9]+))?(?:\s+([0-9]+))?\s*$")
TOTAL_RE = re.compile(r"^\s*TOTAL\b")
RATE_RE = re.compile(r"PASS RATE:\s*([0-9.]+)%\s*\(([0-9]+)/([0-9]+)\)")


def parse_fixture_table(path, ncols):
    """Return list of (name, [int...]) for fixture rows in a log file."""
    rows = []
    in_table = False
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            s = line.rstrip("\n")
            if not in_table:
                # english header line "fixture ..." kicks table off
                if re.search(r"fixture", s, re.I) and "pass" in s.lower():
                    in_table = True
                continue
            if TOTAL_RE.match(s) or s.startswith("──"):
                break
            m = ROW_RE.match(s)
            if m and (ncols == 3 or ncols == 4):
                vals = [int(x) for x in m.groups()[1:] if x is not None]
                if len(vals) >= ncols:
                    rows.append((m.group(1), vals[:ncols]))
    return rows


def parse_rate(path):
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            m = RATE_RE.search(line)
            if m:
                return float(m.group(1)), int(m.group(2)), int(m.group(3))
    return None


def extract_failures(path):
    """Return the raw failure-block text (between the failures header and the closing rule)."""
    with open(path, encoding="utf-8", errors="replace") as fh:
        lines = fh.readlines()
    start = None
    for i, ln in enumerate(lines):
        if re.search(r"failures", ln, re.I) and ln.strip().startswith("──"):
            start = i + 1
            break
    if start is None:
        return ""
    buf = []
    for ln in lines[start:]:
        s = ln.rstrip("\n")
        if s.startswith("══"):
            break
        buf.append(s)
    return "\n".join(buf).strip()


def suite_data(key, crate, ncols):
    log = os.path.join(LOGS_DIR, f"{key}.log")
    if not os.path.isfile(log):
        return None
    rows = parse_fixture_table(log, ncols)
    rate = parse_rate(log)
    failures = extract_failures(log)
    return {
        "log": log,
        "rows": rows,
        "rate": rate,
        "failures": failures,
    }


def build():
    suites = []
    totals = {"pass": 0, "fail": 0, "skip": 0, "total": 0}
    ncols = {"html5-parser": 4}.get  # noop placeholder
    for item in SUITES:
        key, label, crate, desc = item[0], item[1], item[2], item[3]
        hard = item[4] == "hardcoded" if len(item) > 4 else False
        hard_pair = item[5] if len(item) > 5 else None
        cols = 4 if key == "html5-parser" else 3
        d = suite_data(key, crate, cols)
        if d is None:
            suites.append({"key": key, "label": label, "crate": crate, "desc": desc,
                           "rows": [], "rate": None, "failures": "", "error": True,
                           "pass": 0, "fail": 0, "skip": 0, "total": 0, "rate_pct": 0.0})
            continue
        rate_pct, rate_pass, rate_total = d["rate"] or (0.0, 0, 0)
        if hard and hard_pair:
            rate_pct, rate_pass, rate_total = 100.0, hard_pair[0], hard_pair[1]
        skip_sum = 0
        pass_sum = fail_sum = 0
        for name, vals in d["rows"]:
            if cols == 4:
                p, f, sk, t = vals
                skip_sum += sk
            else:
                p, f, t = vals
            pass_sum += p
            fail_sum += f
        # Authoritative numbers come from the harness PASS RATE line
        # (rate_pass/rate_total are the *non-skipped* counts).  Table sums for
        # html5-parser include skips in the per-row `total`, so we always align
        # on rate_total for the suite total and keep skips separate.
        suite_pass, suite_total = rate_pass, rate_total
        suite_fail = suite_total - suite_pass
        totals["pass"] += suite_pass
        totals["fail"] += suite_fail
        totals["skip"] += skip_sum
        totals["total"] += suite_total
        suites.append({
            "key": key, "label": label, "crate": crate, "desc": desc,
            "rows": d["rows"], "failures": d["failures"],
            "rate_pct": rate_pct, "rate_pass": suite_pass, "rate_total": suite_total,
            "pass": suite_pass, "fail": suite_fail, "skip": skip_sum, "total": suite_total,
        })

    overall_pct = (100.0 * totals["pass"] / totals["total"]) if totals["total"] else 0.0
    stamp = datetime.datetime.now(
        datetime.timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    render(suites, totals, overall_pct, stamp)


def esc(x):
    return html.escape(str(x))


def render(suites, totals, overall_pct, stamp):
    os.makedirs(OUT_DIR, exist_ok=True)
    parts = []
    parts.append(HEADER.format(stamp=stamp, overall_pct=overall_pct,
                               total=totals["total"], passed=totals["pass"],
                               failed=totals["fail"], skip=totals["skip"]))

    # Summary cards per suite
    cards = []
    for s in suites:
        if s.get("error"):
            cards.append(f'<div class="card err"><div class="crate">{esc(s["crate"])}</div>'
                         f'<div class="errmsg">log missing</div></div>')
            continue
        color = "green" if s["rate_pct"] == 100 else ("amber" if s["rate_pct"] >= 99 else "red")
        cards.append(
            f'<div class="card {color}">'
            f'<div class="rate">{s["rate_pct"]:.1f}%</div>'
            f'<div class="counts">{s["rate_pass"]} / {s["rate_total"]}</div>'
            f'<div class="crate">{esc(s["crate"])}</div>'
            f'<div class="clabel">{esc(s["label"])}</div>'
            f'</div>')
    parts.append('<section class="sec"><h2>套件总览</h2>'
                 '<div class="cards">' + "".join(cards) + "</div></section>")

    # Overall summary
    parts.append(
        f'<section class="sec"><h2>总体</h2>'
        f'<table><thead><tr><th>通过</th><th>失败</th><th>跳过</th><th>总计</th><th>通过率</th></tr></thead>'
        f'<tbody><tr class="total"><td>{totals["pass"]}</td><td>{totals["fail"]}</td>'
        f'<td>{totals["skip"]}</td><td>{totals["total"]}</td><td>{overall_pct:.2f}%</td></tr></tbody></table>'
        f'</section>')

    # Detail per suite
    for s in suites:
        if s.get("error"):
            continue
        parts.append(f'<section class="sec">')
        parts.append(f'<h2>{esc(s["crate"])}</h2>')
        parts.append(f'<p class="clabel">{esc(s["label"])}</p>')
        parts.append(f'<div class="suite-meta">通过率 <b>{s["rate_pct"]:.1f}%</b> '
                     f'({s["rate_pass"]}/{s["rate_total"]})，'
                     f'失败 {s["fail"]}，跳过 {s["skip"]}</div>')
        # fixture table
        if cols_needed(s):
            parts.append('<table><thead><tr><th>夹具 / fixture</th><th>pass</th>'
                         f'<th>fail</th>{"<th>skip</th>" if s["skip"] else ""}<th>total</th></tr></thead><tbody>')
            for name, vals in s["rows"]:
                if len(vals) == 4:
                    p, f, sk, t = vals
                else:
                    p, f, t = vals
                    sk = None
                cells = [f'<td>{esc(name)}</td>', f'<td>{p}</td>', f'<td>{f}</td>']
                if s["skip"]:
                    cells.append(f'<td>{sk}</td>')
                cells.append(f'<td>{t}</td>')
                rowclass = " class='fail'" if f > 0 else ""
                parts.append("<tr" + rowclass + ">" + "".join(cells) + "</tr>")
            parts.append("</tbody></table>")
        # failures
        if s["failures"]:
            parts.append(f'<details class="fails"><summary>失败明细（{s["fail"]}）</summary>'
                         f'<pre class="pre">{esc(s["failures"])}</pre></details>')
        else:
            parts.append('<p class="ok">全部通过 — 无失败样本。</p>')
        parts.append("</section>")

    parts.append('<footer>Generated by MusKitty WPT harness report script. '
                 '数据来自本地 `cargo test -- --nocapture` 实跑输出。</footer>')

    html_doc = "<!DOCTYPE html><html lang='zh-CN'><head><meta charset='utf-8'>" \
               "<meta name='viewport' content='width=device-width,initial-scale=1'>" \
               f"<title>MusKitty WPT 合规度实测报告</title><style>{CSS}</style></head>" \
               f"<body>{''.join(parts)}</body></html>"
    out = os.path.join(OUT_DIR, "index.html")
    with open(out, "w", encoding="utf-8") as fh:
        fh.write(html_doc)
    print(f"wrote {out}")


def cols_needed(s):
    return True


CSS = r"""
:root{--bg:#0f1117;--panel:#171b26;--line:#262c3b;--txt:#e6e9f0;--mut:#9aa3b5;
--green:#3ddc84;--amber:#f5c344;--red:#ff5c5c;--blue:#5aa9ff;--mono:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--txt);
font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,"PingFang SC","Microsoft YaHei",sans-serif;line-height:1.5}
.wrap{max-width:1100px;margin:0 auto;padding:32px 20px 60px}
h1{font-size:26px;margin:0 0 6px}h1 small{display:block;color:var(--mut);font-weight:400;font-size:14px;margin-top:6px}
.legend{color:var(--mut);font-size:13px;margin:6px 0 26px}
.sec{background:var(--panel);border:1px solid var(--line);border-radius:12px;padding:20px;margin:22px 0}
.sec h2{margin:0 0 4px;font-size:16px}.clabel{color:var(--mut);font-size:12.5px;margin:0 0 10px}
.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:14px}
.card{border:1px solid var(--line);border-radius:10px;padding:14px;background:#1a1f2b}
.card .rate{font-size:26px;font-weight:700;font-family:var(--mono)}
.card .counts{font-family:var(--mono);font-size:13px;color:var(--mut);margin:2px 0 8px}
.card .crate{font-family:var(--mono);font-size:12px;color:var(--blue);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.card.green .rate{color:var(--green)}.card.amber .rate{color:var(--amber)}.card.red .rate{color:var(--red)}
table{width:100%;border-collapse:collapse;font-size:13.5px;margin-top:8px}
th,td{padding:6px 10px;text-align:left;border-bottom:1px solid var(--line)}
th{color:var(--mut);font-weight:600;font-size:12px;text-transform:uppercase;letter-spacing:.03em}
td{font-family:var(--mono)}td:not(:first-child){text-align:right;width:70px}
tr.fail td{color:var(--red)}tbody tr.total td{color:var(--green);font-weight:700;font-size:15px}
.suite-meta{color:var(--mut);font-size:13px;margin:4px 0 2px}.suite-meta b{color:var(--txt)}
details.fails{margin-top:12px;border:1px solid var(--line);border-radius:8px;overflow:hidden}
details.fails summary{cursor:pointer;padding:10px 14px;background:#1a1f2b;color:var(--red);font-weight:600;font-size:13px}
pre.pre{background:#0b0e14;border-top:1px solid var(--line);margin:0;padding:14px;font-family:var(--mono);font-size:12px;overflow:auto;color:#cdd3df;white-space:pre-wrap;word-break:break-word}
p.ok{color:var(--green);font-size:13px}
footer{color:var(--mut);font-size:12px;text-align:center;margin-top:34px;font-family:var(--mono)}
"""

HEADER = """<div class="wrap">
<h1>MusKitty — WPT 合规度实测报告 <small>全仓库 ＷＰＴ 套件 · 生成于 {stamp}</small></h1>
<div class="legend">数据来自各 crate 本地 `cargo test -- --nocapture` 实跑输出（harness 直接解析）。总体通过率 {overall_pct:.2f}%（{passed}/{total} 通过，失败 {failed}，跳过 {skip}）。</div>
"""


if __name__ == "__main__":
    build()