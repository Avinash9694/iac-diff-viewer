//! Recursive JSON diff. Objects by key, arrays by LCS so an inserted element
//! does not cascade into a change on every element after it, and known
//! set-like fields (tags, security group ids, ingress rules, ...) compared
//! without regard to order.

use crate::model::{last_segment, ChangeOp, PropertyChange};
use crate::rename::similarity;
use serde_json::Value;

/// Fields whose array order carries no meaning.
const SET_LIKE_FIELDS: &[&str] = &[
    "Tags",
    "tags",
    "SecurityGroupIds",
    "SecurityGroups",
    "VpcSecurityGroupIds",
    "vpc_security_group_ids",
    "security_group_ids",
    "security_groups",
    "SubnetIds",
    "Subnets",
    "subnet_ids",
    "subnets",
    "AvailabilityZones",
    "availability_zones",
    "ingress",
    "egress",
    "SecurityGroupIngress",
    "SecurityGroupEgress",
    "cidr_blocks",
    "ipv6_cidr_blocks",
    "ManagedPolicyArns",
    "managed_policy_arns",
    "DependsOn",
    "depends_on",
    "Statement",
    "Action",
    "NotAction",
    "Resource",
    "NotResource",
    "Aliases",
    "aliases",
];

/// Beyond this many cell comparisons the LCS table is skipped for a
/// positional diff; nobody has a 500x500 array worth a quadratic pass.
const LCS_CELL_BUDGET: usize = 250_000;

pub fn is_set_like(field: &str) -> bool {
    SET_LIKE_FIELDS.contains(&field)
}

pub fn diff_values(before: &Value, after: &Value) -> Vec<PropertyChange> {
    let mut out = Vec::new();
    walk("", "", before, after, &mut out);
    out
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

fn index(path: &str, i: usize) -> String {
    format!("{path}[{i}]")
}

fn walk(
    before_path: &str,
    after_path: &str,
    before: &Value,
    after: &Value,
    out: &mut Vec<PropertyChange>,
) {
    match (before, after) {
        (Value::Object(b), Value::Object(a)) => {
            let mut keys: Vec<&String> = b.keys().chain(a.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let bp = join(before_path, key);
                let ap = join(after_path, key);
                match (b.get(key), a.get(key)) {
                    (Some(bv), Some(av)) => walk(&bp, &ap, bv, av, out),
                    (Some(bv), None) => out.push(removed(&bp, &ap, bv)),
                    (None, Some(av)) => out.push(added(&ap, av)),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(b), Value::Array(a)) => {
            if is_set_like(last_segment(after_path)) {
                set_diff(before_path, after_path, b, a, out);
            } else {
                lcs_diff(before_path, after_path, b, a, out);
            }
        }
        _ => {
            if before != after {
                out.push(PropertyChange {
                    path: after_path.to_string(),
                    before_path: differing(before_path, after_path),
                    op: ChangeOp::Modified,
                    before: Some(before.clone()),
                    after: Some(after.clone()),
                });
            }
        }
    }
}

fn differing(before_path: &str, after_path: &str) -> Option<String> {
    (before_path != after_path).then(|| before_path.to_string())
}

fn removed(before_path: &str, after_path: &str, value: &Value) -> PropertyChange {
    PropertyChange {
        path: after_path.to_string(),
        before_path: differing(before_path, after_path),
        op: ChangeOp::Removed,
        before: Some(value.clone()),
        after: None,
    }
}

fn added(after_path: &str, value: &Value) -> PropertyChange {
    PropertyChange {
        path: after_path.to_string(),
        before_path: None,
        op: ChangeOp::Added,
        before: None,
        after: Some(value.clone()),
    }
}

/// Leftover object elements of a set-like array at least this similar are
/// treated as the same element edited, so an ingress rule whose CIDR changed
/// reads as one nested change rather than a whole rule removed and re-added.
const SET_ELEMENT_PAIR_THRESHOLD: f64 = 0.5;

const IDENTITY_FIELDS: &[&str] = &["Key", "key", "Sid", "sid", "Name", "name", "Id", "id"];

fn identity(value: &Value) -> Option<String> {
    let obj = value.as_object()?;
    if let Some(found) = IDENTITY_FIELDS.iter().find_map(|f| {
        obj.get(*f)
            .and_then(Value::as_str)
            .map(|v| format!("{f}={v}"))
    }) {
        return Some(found);
    }
    // Security group rules carry no name: protocol + port range is what identifies one.
    let from = obj.get("FromPort").or_else(|| obj.get("from_port"))?;
    let to = obj.get("ToPort").or_else(|| obj.get("to_port"))?;
    let proto = obj
        .get("IpProtocol")
        .or_else(|| obj.get("protocol"))
        .or_else(|| obj.get("ip_protocol"))
        .map(Value::to_string)
        .unwrap_or_default();
    Some(format!("ports={proto}:{from}-{to}"))
}

/// Order-insensitive multiset comparison: equal elements cancel out, similar
/// objects are paired and recursed into, the rest are reported as removed
/// (at their before index) or added (after index).
fn set_diff(
    before_path: &str,
    after_path: &str,
    before: &[Value],
    after: &[Value],
    out: &mut Vec<PropertyChange>,
) {
    let mut matched_before = vec![false; before.len()];
    let mut matched_after = vec![false; after.len()];
    for (bi, bv) in before.iter().enumerate() {
        if let Some(aj) = after
            .iter()
            .enumerate()
            .position(|(aj, av)| !matched_after[aj] && av == bv)
        {
            matched_after[aj] = true;
            matched_before[bi] = true;
        }
    }

    // Identity fields pair first (a tag by its Key, a statement by its Sid),
    // then anything left pairs by structural similarity.
    let mut candidates: Vec<(f64, usize, usize)> = Vec::new();
    for (bi, bv) in before.iter().enumerate() {
        if matched_before[bi] || !bv.is_object() {
            continue;
        }
        for (aj, av) in after.iter().enumerate() {
            if matched_after[aj] || !av.is_object() {
                continue;
            }
            let score = match (identity(bv), identity(av)) {
                (Some(x), Some(y)) if x == y => 2.0,
                (Some(_), Some(_)) => continue,
                _ => similarity(bv, av),
            };
            if score >= SET_ELEMENT_PAIR_THRESHOLD {
                candidates.push((score, bi, aj));
            }
        }
    }
    candidates.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
    for (_, bi, aj) in candidates {
        if matched_before[bi] || matched_after[aj] {
            continue;
        }
        matched_before[bi] = true;
        matched_after[aj] = true;
        walk(
            &index(before_path, bi),
            &index(after_path, aj),
            &before[bi],
            &after[aj],
            out,
        );
    }

    for (bi, bv) in before.iter().enumerate() {
        if !matched_before[bi] {
            out.push(removed(&index(before_path, bi), &index(after_path, bi), bv));
        }
    }
    for (aj, av) in after.iter().enumerate() {
        if !matched_after[aj] {
            out.push(added(&index(after_path, aj), av));
        }
    }
}

fn lcs_diff(
    before_path: &str,
    after_path: &str,
    before: &[Value],
    after: &[Value],
    out: &mut Vec<PropertyChange>,
) {
    let anchors = if before.len() * after.len() > LCS_CELL_BUDGET {
        Vec::new()
    } else {
        lcs_pairs(before, after)
    };
    // Walk the gaps between anchors; inside a gap, pair leftovers positionally
    // and recurse, then report the surplus on either side.
    let mut bi = 0;
    let mut aj = 0;
    for (ab, aa) in anchors
        .iter()
        .copied()
        .chain(std::iter::once((before.len(), after.len())))
    {
        let gap_b = &before[bi..ab];
        let gap_a = &after[aj..aa];
        let paired = gap_b.len().min(gap_a.len());
        for k in 0..paired {
            walk(
                &index(before_path, bi + k),
                &index(after_path, aj + k),
                &gap_b[k],
                &gap_a[k],
                out,
            );
        }
        for (k, bv) in gap_b.iter().enumerate().skip(paired) {
            out.push(removed(
                &index(before_path, bi + k),
                &index(after_path, aj + k),
                bv,
            ));
        }
        for (k, av) in gap_a.iter().enumerate().skip(paired) {
            out.push(added(&index(after_path, aj + k), av));
        }
        bi = ab + 1;
        aj = aa + 1;
    }
}

/// Index pairs `(i, j)` of a longest common subsequence under `==`.
fn lcs_pairs(before: &[Value], after: &[Value]) -> Vec<(usize, usize)> {
    let (n, m) = (before.len(), after.len());
    if n == 0 || m == 0 {
        return Vec::new();
    }
    let mut table = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i][j] = if before[i] == after[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if before[i] == after[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Side;
    use serde_json::json;

    fn paths(changes: &[PropertyChange]) -> Vec<(String, ChangeOp)> {
        changes.iter().map(|c| (c.path.clone(), c.op)).collect()
    }

    #[test]
    fn identical_values_produce_no_changes() {
        let v = json!({"a": [1, {"b": "c"}], "d": null});
        assert!(diff_values(&v, &v).is_empty());
    }

    #[test]
    fn object_keys_added_removed_modified() {
        let changes = diff_values(
            &json!({"keep": 1, "gone": 2, "changed": "x"}),
            &json!({"keep": 1, "new": 3, "changed": "y"}),
        );
        assert_eq!(
            paths(&changes),
            vec![
                ("changed".to_string(), ChangeOp::Modified),
                ("gone".to_string(), ChangeOp::Removed),
                ("new".to_string(), ChangeOp::Added),
            ]
        );
        assert_eq!(changes[0].before, Some(json!("x")));
        assert_eq!(changes[0].after, Some(json!("y")));
    }

    #[test]
    fn nested_paths_are_dotted() {
        let changes = diff_values(
            &json!({"VpcConfig": {"Subnets": "a", "Nested": {"Deep": 1}}}),
            &json!({"VpcConfig": {"Subnets": "a", "Nested": {"Deep": 2}}}),
        );
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "VpcConfig.Nested.Deep");
    }

    #[test]
    fn type_change_is_a_single_modified_leaf() {
        let changes = diff_values(&json!({"a": {"b": 1}}), &json!({"a": "scalar"}));
        assert_eq!(paths(&changes), vec![("a".to_string(), ChangeOp::Modified)]);
    }

    #[test]
    fn array_insert_does_not_cascade() {
        let changes = diff_values(
            &json!({"Steps": ["a", "b", "c"]}),
            &json!({"Steps": ["a", "x", "b", "c"]}),
        );
        assert_eq!(
            paths(&changes),
            vec![("Steps[1]".to_string(), ChangeOp::Added)]
        );
    }

    #[test]
    fn array_delete_does_not_cascade() {
        let changes = diff_values(
            &json!({"Steps": ["a", "b", "c", "d"]}),
            &json!({"Steps": ["a", "c", "d"]}),
        );
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].op, ChangeOp::Removed);
        assert_eq!(changes[0].path_on(Side::Before), "Steps[1]");
        assert_eq!(changes[0].before, Some(json!("b")));
    }

    #[test]
    fn set_like_pairs_similar_objects_and_recurses() {
        let changes = diff_values(
            &json!({"Tags": [{"Key": "env", "Value": "dev"}, {"Key": "team", "Value": "web"}]}),
            &json!({"Tags": [{"Key": "team", "Value": "web"}, {"Key": "env", "Value": "prod"}]}),
        );
        assert_eq!(
            paths(&changes),
            vec![("Tags[1].Value".to_string(), ChangeOp::Modified)]
        );
        assert_eq!(changes[0].before_path.as_deref(), Some("Tags[0].Value"));
    }

    #[test]
    fn set_like_unrelated_objects_are_not_paired() {
        let changes = diff_values(
            &json!({"Statement": [{"Effect": "Allow", "Action": "s3:GetObject", "Resource": "a"}]}),
            &json!({"Statement": [{"Sid": "x", "Principal": "*", "Condition": {"Bool": {}}}]}),
        );
        assert_eq!(
            paths(&changes),
            vec![
                ("Statement[0]".to_string(), ChangeOp::Removed),
                ("Statement[0]".to_string(), ChangeOp::Added),
            ]
        );
    }

    #[test]
    fn array_element_modified_in_place_recurses() {
        let changes = diff_values(
            &json!({"Rules": [{"Port": 80}, {"Port": 443}]}),
            &json!({"Rules": [{"Port": 80}, {"Port": 8443}]}),
        );
        assert_eq!(
            paths(&changes),
            vec![("Rules[1].Port".to_string(), ChangeOp::Modified)]
        );
    }

    #[test]
    fn shifted_element_modified_reports_both_paths() {
        let changes = diff_values(
            &json!({"Rules": [{"Port": 80}, {"Port": 443}]}),
            &json!({"Rules": [{"Port": 22}, {"Port": 80}, {"Port": 8443}]}),
        );
        let modified: Vec<_> = changes
            .iter()
            .filter(|c| c.op == ChangeOp::Modified)
            .collect();
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].path, "Rules[2].Port");
        assert_eq!(modified[0].before_path.as_deref(), Some("Rules[1].Port"));
        assert!(changes
            .iter()
            .any(|c| c.path == "Rules[0]" && c.op == ChangeOp::Added));
    }

    #[test]
    fn set_like_field_ignores_order() {
        let changes = diff_values(
            &json!({"SecurityGroupIds": ["sg-1", "sg-2", "sg-3"]}),
            &json!({"SecurityGroupIds": ["sg-3", "sg-1", "sg-2"]}),
        );
        assert!(changes.is_empty());
    }

    #[test]
    fn set_like_field_reports_membership_changes_only() {
        let changes = diff_values(
            &json!({"Tags": [{"Key": "a", "Value": "1"}, {"Key": "b", "Value": "2"}]}),
            &json!({"Tags": [{"Key": "b", "Value": "2"}, {"Key": "c", "Value": "3"}]}),
        );
        assert_eq!(
            paths(&changes),
            vec![
                ("Tags[0]".to_string(), ChangeOp::Removed),
                ("Tags[1]".to_string(), ChangeOp::Added),
            ]
        );
    }

    #[test]
    fn set_like_handles_duplicates_as_multiset() {
        let changes = diff_values(
            &json!({"cidr_blocks": ["10.0.0.0/8", "10.0.0.0/8"]}),
            &json!({"cidr_blocks": ["10.0.0.0/8"]}),
        );
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].op, ChangeOp::Removed);
    }

    #[test]
    fn nested_set_like_under_ingress() {
        let changes = diff_values(
            &json!({"ingress": [{"from_port": 22, "cidr_blocks": ["10.0.0.0/8"]}]}),
            &json!({"ingress": [{"from_port": 22, "cidr_blocks": ["10.0.0.0/8", "0.0.0.0/0"]}]}),
        );
        assert_eq!(
            paths(&changes),
            vec![("ingress[0].cidr_blocks[1]".to_string(), ChangeOp::Added)]
        );
    }

    #[test]
    fn ordered_arrays_still_respect_order() {
        let changes = diff_values(&json!({"Cmd": ["a", "b"]}), &json!({"Cmd": ["b", "a"]}));
        assert!(!changes.is_empty());
    }

    #[test]
    fn empty_and_missing_differ() {
        let changes = diff_values(&json!({"a": []}), &json!({}));
        assert_eq!(paths(&changes), vec![("a".to_string(), ChangeOp::Removed)]);
    }

    #[test]
    fn lcs_pairs_picks_longest_subsequence() {
        let b: Vec<Value> = ["a", "b", "c", "d"].iter().map(|s| json!(s)).collect();
        let a: Vec<Value> = ["b", "d", "a"].iter().map(|s| json!(s)).collect();
        assert_eq!(lcs_pairs(&b, &a), vec![(1, 0), (3, 1)]);
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ingress_rules_pair_by_protocol_and_port_range() {
        let changes = diff_values(
            &json!({"SecurityGroupIngress": [
                {"IpProtocol": "tcp", "FromPort": 443, "ToPort": 443, "CidrIp": "0.0.0.0/0"},
                {"IpProtocol": "tcp", "FromPort": 22, "ToPort": 22, "CidrIp": "10.0.0.0/8", "Description": "corp"}
            ]}),
            &json!({"SecurityGroupIngress": [
                {"IpProtocol": "tcp", "FromPort": 443, "ToPort": 443, "CidrIp": "0.0.0.0/0"},
                {"IpProtocol": "tcp", "FromPort": 22, "ToPort": 22, "CidrIp": "0.0.0.0/0", "Description": "temporary"}
            ]}),
        );
        let paths: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "SecurityGroupIngress[1].CidrIp",
                "SecurityGroupIngress[1].Description"
            ]
        );
        assert!(changes.iter().all(|c| c.op == ChangeOp::Modified));
    }

    #[test]
    fn different_port_ranges_are_different_rules() {
        let changes = diff_values(
            &json!({"ingress": [{"protocol": "tcp", "from_port": 22, "to_port": 22, "cidr_blocks": ["10.0.0.0/8"]}]}),
            &json!({"ingress": [{"protocol": "tcp", "from_port": 80, "to_port": 80, "cidr_blocks": ["10.0.0.0/8"]}]}),
        );
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].op, ChangeOp::Removed);
        assert_eq!(changes[1].op, ChangeOp::Added);
    }
}
