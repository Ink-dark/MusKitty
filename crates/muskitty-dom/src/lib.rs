//! MusKitty DOM
//!
//! 实现 DOM Living Standard 的核心节点类型与树操作 API。
//!
//! # 模块结构
//!
//! - [`node`]：`Node`、`NodeType`、`NodeKind`、`Descendants`
//! - [`element`] / [`text`] / [`comment`] / [`document`] / [`document_type`] /
//!   [`document_fragment`]：各节点类型的数据载体
//! - [`attribute`]：`Attribute`、`Namespace`
//! - [`tree`]：树修改算法（`append_child` / `insert_before` / `remove_child` / `replace_child`）
//! - [`error`]：`DomError`
//!
//! # 参考
//!
//! - DOM Living Standard: <https://dom.spec.whatwg.org/>

pub mod attribute;
pub mod comment;
pub mod document;
pub mod document_fragment;
pub mod document_type;
pub mod element;
pub mod error;
pub mod node;
pub mod processing_instruction;
pub mod text;
pub mod tree;

pub use attribute::{Attribute, Namespace};
pub use comment::CommentData;
pub use document::DocumentData;
pub use document_fragment::DocumentFragmentData;
pub use document_type::DocumentTypeData;
pub use element::ElementData;
pub use error::DomError;
pub use node::{Descendants, Node, NodeKind, NodeType};
pub use processing_instruction::ProcessingInstructionData;
pub use text::TextData;
pub use tree::{append_child, insert_before, remove_child, replace_child, set_text_content};
