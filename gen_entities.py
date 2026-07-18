#!/usr/bin/env python3
"""Generate muskitty-html5-parser/src/tokenizer/entities.rs from WHATWG entities.json"""
import json, sys

with open('temp_entities.json', encoding='utf-8') as f:
    entities = json.load(f)

def escape_for_rust(s):
    """Escape a Python str into a Rust string literal body (ASCII-only,
    non-ASCII emitted as `\\u{XXXX}` to match the existing entities.rs style)."""
    out = []
    for ch in s:
        o = ord(ch)
        if ch == '\\':
            out.append('\\\\')
        elif ch == '"':
            out.append('\\"')
        elif 0x20 <= o <= 0x7E:
            out.append(ch)
        else:
            out.append(f'\\u{{{o:04X}}}')
    return ''.join(out)


entries = []
for full_name, val in entities.items():
    name = full_name[1:]  # strip leading &
    codepoints = val['codepoints']
    chars = ''.join(chr(cp) for cp in codepoints)
    entries.append((name, chars))

entries.sort(key=lambda x: x[0])

lines = []
lines.append('// Generated from WHATWG entities.json. DO NOT EDIT BY HAND.')
lines.append('// Entity names preserve the trailing `;` exactly as in the spec:')
lines.append('//   - non-legacy entities are stored as `Name;` (semicolon required)')
lines.append('//   - legacy entities are stored BOTH as `Name` and `Name;`')
lines.append(f'// {len(entries)} entries, sorted alphabetically for binary search.')
lines.append('')
lines.append('/// Sorted array of (entity_name, resolved_characters).')
lines.append('/// Entity name does NOT include leading `&` but DOES keep a')
lines.append('/// trailing `;` when the spec requires one (i.e. non-legacy).')
lines.append('pub(crate) static ENTITIES: &[(&str, &str)] = &[')

for name, chars in entries:
    escaped = escape_for_rust(chars)
    lines.append(f'    ("{name}", "{escaped}"),')

lines.append('];')
lines.append('')
lines.append('pub(crate) fn resolve_named_entity(name: &str) -> Option<&\'static str> {')
lines.append('    ENTITIES.binary_search_by_key(&name, |&(n, _)| n)')
lines.append('        .ok()')
lines.append('        .map(|idx| ENTITIES[idx].1)')
lines.append('}')

with open('crates/muskitty-html5-parser/src/tokenizer/entities.rs', 'w', encoding='utf-8') as f:
    f.write('\n'.join(lines))

multi = sum(1 for _, c in entries if len(c) > 1)
print(f'✅ Generated {len(entries)} entity entries ({multi} multi-char)')
print(f'   Output: crates/muskitty-html5-parser/src/tokenizer/entities.rs')
