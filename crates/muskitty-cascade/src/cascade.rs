//! §6.1 Cascade 排序算法。
//!
//! 规范源: `d:\csswg\css-cascade-5\Overview.md` §6.1 L855-994
//!
//! 按 4 个准则降序排序 declared values（推迟了 Context/Scope/Layers）：
//! 1. Origin and Importance（§6.1 准则 1）
//! 2. Element-Attached Styles（§6.1 准则 4）
//! 3. Specificity（§6.1 准则 6）
//! 4. Order of Appearance（§6.1 准则 7）
//!
//! 输出是按属性分组的有序列表（首项为 cascade 胜出者）。
//! 保留完整列表是为了支持 `revert*` 关键字（推迟）。

use crate::style::DeclaredValue;
use muskitty_cssom::Origin;
use std::collections::HashMap;

/// §6.1: 对 declared values 按 cascade 准则排序。
///
/// 按 property 分组，每组内按 §6.1 的 4 个准则降序排序。
/// 返回每属性的有序列表（首项为胜出者）。
pub fn cascade_for_element(declared: Vec<DeclaredValue>) -> HashMap<String, Vec<DeclaredValue>> {
    let mut groups: HashMap<String, Vec<DeclaredValue>> = HashMap::new();
    for d in declared {
        groups.entry(d.property.clone()).or_default().push(d);
    }

    for group in groups.values_mut() {
        // 降序排序：sort_key 大的在前（用 Reverse 实现降序）
        group.sort_by_key(|d| std::cmp::Reverse(cascade_sort_key(d)));
    }

    groups
}

/// §6.1: 获取某属性 cascade 胜出者（有序列表首项）。
pub fn cascade_winner(group: &[DeclaredValue]) -> Option<&DeclaredValue> {
    group.first()
}

/// §6.1: cascade 排序 key（降序比较）。
///
/// 返回 tuple，按 lexicographic 降序比较：
/// 1. Origin × Importance（6 级）
/// 2. Element-Attached Styles（style attr 优先）
/// 3. Specificity（(A, B, C) 降序）
/// 4. Order（文档序，后出现的优先）
fn cascade_sort_key(d: &DeclaredValue) -> (u8, u8, (u32, u32, u32), usize) {
    // §6.1 准则 1: Origin and Importance
    // 优先级从高到低（推迟 Transition/Animation）：
    //   Important UA (6) > Important User (5) > Important Author (4)
    //   > Normal Author (3) > Normal User (2) > Normal UA (1)
    let origin_importance: u8 = match (d.origin, d.important) {
        (Origin::UserAgent, true) => 6,
        (Origin::User, true) => 5,
        (Origin::Author, true) => 4,
        (Origin::Author, false) => 3,
        (Origin::User, false) => 2,
        (Origin::UserAgent, false) => 1,
    };

    // §6.1 准则 4: Element-Attached Styles
    let style_attr: u8 = if d.from_style_attr { 1 } else { 0 };

    // §6.1 准则 6: Specificity
    let spec = (d.specificity.a, d.specificity.b, d.specificity.c);

    // §6.1 准则 7: Order of Appearance（后出现的胜出，大值优先）
    let order = d.order;

    (origin_importance, style_attr, spec, order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use muskitty_css::parser::ComponentValue;
    use muskitty_css::tokenizer::Token;
    use muskitty_selectors::Specificity;

    fn make_decl(
        property: &str,
        origin: Origin,
        important: bool,
        specificity: Specificity,
        order: usize,
    ) -> DeclaredValue {
        DeclaredValue {
            property: property.to_string(),
            value: vec![ComponentValue::PreservedToken(Token::Ident(
                "red".to_string(),
            ))],
            important,
            origin,
            specificity,
            order,
            from_style_attr: false,
        }
    }

    #[test]
    fn single_declaration_returns_one_group() {
        let declared = vec![make_decl(
            "color",
            Origin::Author,
            false,
            Specificity::default(),
            1,
        )];
        let groups = cascade_for_element(declared);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups["color"].len(), 1);
    }

    #[test]
    fn higher_specificity_wins() {
        let declared = vec![
            make_decl(
                "color",
                Origin::Author,
                false,
                Specificity { a: 0, b: 0, c: 1 },
                1,
            ),
            make_decl(
                "color",
                Origin::Author,
                false,
                Specificity { a: 0, b: 1, c: 0 },
                2,
            ),
        ];
        let groups = cascade_for_element(declared);
        let winner = cascade_winner(&groups["color"]).unwrap();
        assert_eq!(winner.specificity.b, 1);
    }

    #[test]
    fn important_beats_normal_same_origin() {
        let declared = vec![
            make_decl(
                "color",
                Origin::Author,
                false,
                Specificity { a: 1, b: 0, c: 0 },
                1,
            ),
            make_decl(
                "color",
                Origin::Author,
                true,
                Specificity { a: 0, b: 0, c: 1 },
                2,
            ),
        ];
        let groups = cascade_for_element(declared);
        let winner = cascade_winner(&groups["color"]).unwrap();
        assert!(winner.important);
    }

    #[test]
    fn normal_author_beats_normal_user() {
        let declared = vec![
            make_decl(
                "color",
                Origin::User,
                false,
                Specificity { a: 1, b: 0, c: 0 },
                1,
            ),
            make_decl(
                "color",
                Origin::Author,
                false,
                Specificity { a: 0, b: 0, c: 1 },
                2,
            ),
        ];
        let groups = cascade_for_element(declared);
        let winner = cascade_winner(&groups["color"]).unwrap();
        assert_eq!(winner.origin, Origin::Author);
    }

    #[test]
    fn important_ua_beats_important_author() {
        let declared = vec![
            make_decl(
                "color",
                Origin::Author,
                true,
                Specificity { a: 1, b: 0, c: 0 },
                1,
            ),
            make_decl(
                "color",
                Origin::UserAgent,
                true,
                Specificity { a: 0, b: 0, c: 1 },
                2,
            ),
        ];
        let groups = cascade_for_element(declared);
        let winner = cascade_winner(&groups["color"]).unwrap();
        assert_eq!(winner.origin, Origin::UserAgent);
    }

    #[test]
    fn later_order_wins_on_tie() {
        let declared = vec![
            make_decl("color", Origin::Author, false, Specificity::default(), 1),
            make_decl("color", Origin::Author, false, Specificity::default(), 2),
        ];
        let groups = cascade_for_element(declared);
        let winner = cascade_winner(&groups["color"]).unwrap();
        assert_eq!(winner.order, 2);
    }

    #[test]
    fn style_attr_beats_same_specificity() {
        let mut d1 = make_decl("color", Origin::Author, false, Specificity::default(), 1);
        let d2 = make_decl("color", Origin::Author, false, Specificity::default(), 2);
        d1.from_style_attr = true;

        let groups = cascade_for_element(vec![d1, d2]);
        let winner = cascade_winner(&groups["color"]).unwrap();
        assert!(winner.from_style_attr);
    }

    #[test]
    fn different_properties_grouped_separately() {
        let declared = vec![
            make_decl("color", Origin::Author, false, Specificity::default(), 1),
            make_decl(
                "font-size",
                Origin::Author,
                false,
                Specificity::default(),
                2,
            ),
        ];
        let groups = cascade_for_element(declared);
        assert_eq!(groups.len(), 2);
        assert!(groups.contains_key("color"));
        assert!(groups.contains_key("font-size"));
    }

    #[test]
    fn full_sort_order_verified() {
        // 测试完整排序链：important > normal, author > user > ua
        let declared = vec![
            // Normal UA
            make_decl("color", Origin::UserAgent, false, Specificity::default(), 1),
            // Normal User
            make_decl("color", Origin::User, false, Specificity::default(), 2),
            // Normal Author
            make_decl("color", Origin::Author, false, Specificity::default(), 3),
            // Important Author
            make_decl("color", Origin::Author, true, Specificity::default(), 4),
            // Important User
            make_decl("color", Origin::User, true, Specificity::default(), 5),
            // Important UA
            make_decl("color", Origin::UserAgent, true, Specificity::default(), 6),
        ];
        let groups = cascade_for_element(declared);
        let group = &groups["color"];
        assert_eq!(group.len(), 6);
        // 排序后：Important UA > Important User > Important Author > Normal Author > Normal User > Normal UA
        assert_eq!(group[0].origin, Origin::UserAgent);
        assert!(group[0].important);
        assert_eq!(group[1].origin, Origin::User);
        assert!(group[1].important);
        assert_eq!(group[2].origin, Origin::Author);
        assert!(group[2].important);
        assert_eq!(group[3].origin, Origin::Author);
        assert!(!group[3].important);
        assert_eq!(group[4].origin, Origin::User);
        assert!(!group[4].important);
        assert_eq!(group[5].origin, Origin::UserAgent);
        assert!(!group[5].important);
    }
}
