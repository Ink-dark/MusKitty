# MusKitty HTML Parser — Progress Dashboard

> 最后更新: 2026-07-11 | 基于 git commit `5ec53bf`

## 总览

| 模块 | 状态 | 规范覆盖 | 备注 |
|------|------|---------|------|
| **Tokenizer** | ✅ 全部完成 | §13.2.5.1–§13.2.5.80 (80/80) | 所有 80 个状态已实现 |
| Parser (Tree Construction) | ⬜ 预留 | §13.2.6 | 仅 mod.rs 占位 |
| DOM | ⬜ 预留 | — | 仅 mod.rs 占位 |
| Error | ⬜ 预留 | — | 仅 mod.rs 占位 |

## Tokenizer 进度详情

### Token 类型 (5/5) ✅

| Token | 位置 | 状态 |
|-------|------|------|
| `Doctype(DoctypeToken)` | `types.rs:11-13` | ✅ 已实现 |
| `Tag(TagToken)` | `types.rs:14` | ✅ 已实现 |
| `Comment(String)` | `types.rs:16` | ✅ 已实现 |
| `Character(char)` | `types.rs:18` | ✅ 已实现 |
| `EOF` | `types.rs:20` | ✅ 已实现 |

### 状态实现 (80/80) ✅

#### 内容模型状态 (5/5) §13.2.5.1–§13.2.5.5
| # | 状态 | 规范 | 实现 | 状态 |
|---|------|------|------|------|
| 1 | Data | §13.2.5.1 | `handle_data_state` | ✅ |
| 2 | RCDATA | §13.2.5.2 | `handle_rcdata_state` | ✅ |
| 3 | RAWTEXT | §13.2.5.3 | `handle_rawtext_state` | ✅ |
| 4 | ScriptData | §13.2.5.4 | `handle_script_data_state` | ✅ |
| 5 | PLAINTEXT | §13.2.5.5 | `handle_plaintext_state` | ✅ |

#### 标签开关状态 (4/4) §13.2.5.6–§13.2.5.8, §13.2.5.40
| 6 | TagOpen | §13.2.5.6 | `handle_tag_open_state` | ✅ |
| 7 | EndTagOpen | §13.2.5.7 | `handle_end_tag_open_state` | ✅ |
| 8 | TagName | §13.2.5.8 | `handle_tag_name_state` | ✅ |
| 40 | SelfClosingStartTag | §13.2.5.40 | `handle_self_closing_start_tag_state` | ✅ |

#### RCDATA 状态 (3/3) §13.2.5.9–§13.2.5.11
| 9 | RCDATALessThanSign | §13.2.5.9 | ✅ |
| 10 | RCDATAEndTagOpen | §13.2.5.10 | ✅ |
| 11 | RCDATAEndTagName | §13.2.5.11 | ✅ |

#### RAWTEXT 状态 (3/3) §13.2.5.12–§13.2.5.14
| 12 | RAWTEXTLessThanSign | §13.2.5.12 | ✅ |
| 13 | RAWTEXTEndTagOpen | §13.2.5.13 | ✅ |
| 14 | RAWTEXTEndTagName | §13.2.5.14 | ✅ |

#### Script Data 状态 (17/17) §13.2.5.15–§13.2.5.31
| 15 | ScriptDataLessThanSign | §13.2.5.15 | ✅ |
| 16 | ScriptDataEndTagOpen | §13.2.5.16 | ✅ |
| 17 | ScriptDataEndTagName | §13.2.5.17 | ✅ |
| 18 | ScriptDataEscapeStart | §13.2.5.18 | ✅ |
| 19 | ScriptDataEscapeStartDash | §13.2.5.19 | ✅ |
| 20 | ScriptDataEscaped | §13.2.5.20 | ✅ |
| 21 | ScriptDataEscapedDash | §13.2.5.21 | ✅ |
| 22 | ScriptDataEscapedDashDash | §13.2.5.22 | ✅ |
| 23 | ScriptDataEscapedLessThanSign | §13.2.5.23 | ✅ |
| 24 | ScriptDataEscapedEndTagOpen | §13.2.5.24 | ✅ |
| 25 | ScriptDataEscapedEndTagName | §13.2.5.25 | ✅ |
| 26 | ScriptDataDoubleEscapeStart | §13.2.5.26 | ✅ |
| 27 | ScriptDataDoubleEscaped | §13.2.5.27 | ✅ |
| 28 | ScriptDataDoubleEscapedDash | §13.2.5.28 | ✅ |
| 29 | ScriptDataDoubleEscapedDashDash | §13.2.5.29 | ✅ |
| 30 | ScriptDataDoubleEscapedLessThanSign | §13.2.5.30 | ✅ |
| 31 | ScriptDataDoubleEscapeEnd | §13.2.5.31 | ✅ |

#### 属性状态 (9/9) §13.2.5.32–§13.2.5.40
| 32 | BeforeAttributeName | §13.2.5.32 | ✅ |
| 33 | AttributeName | §13.2.5.33 | ✅ |
| 34 | AfterAttributeName | §13.2.5.34 | ✅ |
| 35 | BeforeAttributeValue | §13.2.5.35 | ✅ |
| 36 | AttributeValueDoubleQuoted | §13.2.5.36 | ✅ |
| 37 | AttributeValueSingleQuoted | §13.2.5.37 | ✅ |
| 38 | AttributeValueUnquoted | §13.2.5.38 | ✅ |
| 39 | AfterAttributeValueQuoted | §13.2.5.39 | ✅ |

#### 注释状态 (12/12) §13.2.5.41–§13.2.5.52
| 41 | BogusComment | §13.2.5.41 | ✅ |
| 42 | MarkupDeclarationOpen | §13.2.5.42 | ✅ |
| 43 | CommentStart | §13.2.5.43 | ✅ |
| 44 | CommentStartDash | §13.2.5.44 | ✅ |
| 45 | Comment | §13.2.5.45 | ✅ |
| 46 | CommentLessThanSign | §13.2.5.46 | ✅ |
| 47 | CommentLessThanSignBang | §13.2.5.47 | ✅ |
| 48 | CommentLessThanSignBangDash | §13.2.5.48 | ✅ |
| 49 | CommentLessThanSignBangDashDash | §13.2.5.49 | ✅ |
| 50 | CommentEndDash | §13.2.5.50 | ✅ |
| 51 | CommentEnd | §13.2.5.51 | ✅ |
| 52 | CommentEndBang | §13.2.5.52 | ✅ |

#### DOCTYPE 状态 (16/16) §13.2.5.53–§13.2.5.68
| 53 | Doctype | §13.2.5.53 | ✅ |
| 54 | BeforeDoctypeName | §13.2.5.54 | ✅ |
| 55 | DoctypeName | §13.2.5.55 | ✅ |
| 56 | AfterDoctypeName | §13.2.5.56 | ✅ |
| 57 | AfterDoctypePublicKeyword | §13.2.5.57 | ✅ |
| 58 | BeforeDoctypePublicId | §13.2.5.58 | ✅ |
| 59 | DoctypePublicIdDoubleQuoted | §13.2.5.59 | ✅ |
| 60 | DoctypePublicIdSingleQuoted | §13.2.5.60 | ✅ |
| 61 | AfterDoctypePublicId | §13.2.5.61 | ✅ |
| 62 | BetweenDoctypePublicAndSystemIds | §13.2.5.62 | ✅ |
| 63 | AfterDoctypeSystemKeyword | §13.2.5.63 | ✅ |
| 64 | BeforeDoctypeSystemId | §13.2.5.64 | ✅ |
| 65 | DoctypeSystemIdDoubleQuoted | §13.2.5.65 | ✅ |
| 66 | DoctypeSystemIdSingleQuoted | §13.2.5.66 | ✅ |
| 67 | AfterDoctypeSystemId | §13.2.5.67 | ✅ |
| 68 | BogusDoctype | §13.2.5.68 | ✅ |

#### CDATA Section 状态 (3/3) §13.2.5.69–§13.2.5.71
| 69 | CDATASection | §13.2.5.69 | ✅ |
| 70 | CDATASectionBracket | §13.2.5.70 | ✅ |
| 71 | CDATASectionEnd | §13.2.5.71 | ✅ |

#### Character Reference 状态 (9/9) §13.2.5.72–§13.2.5.80
| 72 | CharacterReference | §13.2.5.72 | ✅ |
| 73 | NamedCharacterReference | §13.2.5.73 | ✅ |
| 74 | AmbiguousAmpersand | §13.2.5.74 | ✅ |
| 75 | NumericCharacterReference | §13.2.5.75 | ✅ |
| 76 | HexCharacterReferenceStart | §13.2.5.76 | ✅ |
| 77 | DecimalCharacterReferenceStart | §13.2.5.77 | ✅ |
| 78 | HexCharacterReference | §13.2.5.78 | ✅ |
| 79 | DecimalCharacterReference | §13.2.5.79 | ✅ |
| 80 | NumericCharacterReferenceEnd | §13.2.5.80 | ✅ |

### 辅助基础设施

| 功能 | 文件 | 状态 |
|------|------|------|
| Tokenizer trait | `trait_def.rs` | ✅ |
| HtmlTokenizer struct | `impls.rs:24-69` | ✅ |
| reconsume 机制 | `impls.rs:126-138` | ✅ |
| pending_tokens 多 token 发射 | `impls.rs:61` | ✅ |
| 状态设置/查询 | `impls.rs:255-261` | ✅ |
| reset() | `impls.rs:263-283` | ✅ |
| appropriate_end_tag_name | `impls.rs:54,153-155` | ✅ |
| temporary_buffer (RCDATA/RAWTEXT/Script) | `impls.rs:57` | ✅ |
| return_state (字符引用回退) | `impls.rs:65` | ✅ |
| character_reference_code | `impls.rs:68` | ✅ |
| 全量 WHATWG 命名实体表 | `entities.rs` | ✅ 2,231 条 |
| 实体查找 (二分搜索) | `entities.rs` | ✅ |
| Windows-1252 替换表 | `impls.rs:1852-1878` | ✅ |

## 源代码结构

```
muskitty-html-parser/
├── Cargo.toml              (零外部依赖，仅 std)
└── src/
    ├── lib.rs              (crate root)
    ├── tokenizer/
    │   ├── mod.rs          (模块声明，pub re-export)
    │   ├── types.rs        (Token, TagToken, DoctypeToken, State 枚举)
    │   ├── trait_def.rs    (Tokenizer trait)
    │   ├── impls.rs        (HtmlTokenizer 实现 + 全部测试, ~5324 行)
    │   └── entities.rs     (2,231 条 WHATWG 命名实体表, ~2244 行)
    ├── parser/mod.rs       (预留)
    ├── dom/mod.rs          (预留)
    └── error/mod.rs        (预留)
```

## 测试覆盖

- 测试代码: `impls.rs` 行 3333–5323 (~1991 行)
- 内联单元测试，覆盖所有状态的主要路径
- 测试分类:
  - Data state (基础字符、EOF、NULL、字符引用)
  - TagOpen / EndTagOpen / TagName (大小写、非 ASCII、空字符)
  - SelfClosingStartTag
  - MarkupDeclarationOpen (Comment/DOCTYPE/CDATA/Bogus 分发)
  - Comment 系列 (CommentStart, Comment, CommentLessThanSign, CommentEnd, CommentEndBang)
  - BogusComment (从 `<?` 和 `<!` 两条路径)
  - Attribute 系列 (双引号/单引号/无引号、布尔属性、多个属性、非 ASCII)
  - RCDATA / RAWTEXT / PLAINTEXT 内容模型
  - ScriptData 系列 (end tag match/backout, escape start, escaped, double escaped)
  - DOCTYPE 系列 (name, PUBLIC/SYSTEM identifier, force-quirks, BogusDoctype)
  - Character Reference 系列 (named, decimal, hex, null, Windows-1252, ambiguous)
  - CDATA Section

## 已知待办 (TODO)

从代码中的 `// TODO:` 标记:

1. **Parse Error 记录**: 代码中大量 `// TODO: record parse error (...)` 标记，当前 tokenizer 正确处理了字符级行为，但尚未实现 parse error 的收集和上报机制（需要 `error` 模块配合）
2. **Parser (Tree Construction)**: §13.2.6 尚未开始，仅有占位文件
3. **DOM 模块**: 尚未开始
4. **WPT 语义比对**: 需要架构师执行

## Git 提交历史

```
5ec53bf Expand named entity table to full WHATWG set + CDATA completion
9f477e1 Implement CDATA Section states (§13.2.5.69–§13.2.5.71)
6b09c5b Implement Character Reference states (§13.2.5.72–§13.2.5.80)
eb9be30 Implement Script Data states (§13.2.5.4, §13.2.5.15–§13.2.5.31)
7a94844 Implement RCDATA/RAWTEXT/PLAINTEXT content model states + tag detection
e482c8b [tokenizer] implement DOCTYPE PUBLIC + SYSTEM identifier states
b34d457 [tokenizer] implement AfterDoctypeName + BogusDoctype
5ab2ccf [tokenizer] implement BeforeDoctypeName + DoctypeName
ec60b2f [tokenizer] add current_doctype field + implement Doctype entry state
8de7acd [tokenizer] implement CommentLessThanSign and CommentEnd series
8cda715 [tokenizer] implement CommentStart, CommentStartDash, Comment states
881fcaa [tokenizer] implement BogusComment state
20a1e8a [tokenizer] implement MarkupDeclarationOpen state
46f0215 [tokenizer] implement attribute states
1cdbae1 [tokenizer] fix missing return in next_token()
c08452a [tokenizer] implement EndTagOpen, TagName, SelfClosingStartTag
fd264b7 [tokenizer] implement TagOpen state with reconsume
350b642 [tokenizer] implement Data state with unit tests
9d7a044 [tokenizer] revise types per spec review
91088c9 [init] add CLAUDE.md
5373001 [init] project scaffold + Token/TagToken/Tokenizer type defs
```

## 下一步

1. **Error 模块**: 实现 ParseError 类型和收集机制，让 tokenizer 的所有 `TODO: record parse error` 落地
2. **Parser 模块**: 开始 §13.2.6 Tree Construction 实现
3. **WPT 对接**: 架构师跑 WPT 语义比对
