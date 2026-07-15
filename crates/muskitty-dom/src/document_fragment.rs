//! DocumentFragment 节点数据。
//!
//! 参见 DOM Living Standard §9 (Interface DocumentFragment)。

/// `DocumentFragment` 节点的数据载体。
///
/// DocumentFragment 是轻量级的"最小文档"容器，本身无额外字段。
#[derive(Debug, Clone, Default)]
pub struct DocumentFragmentData;
