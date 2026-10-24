//! A removed and an added resource of the same type whose properties are
//! almost identical is a rename (CloudFormation logical id) or a move
//! (Terraform address), not two unrelated changes.

use crate::model::Resource;
use serde_json::Value;
use std::collections::HashSet;

pub const RENAME_SIMILARITY_THRESHOLD: f64 = 0.9;

/// Jaccard similarity over flattened `path=value` leaves.
pub fn similarity(a: &Value, b: &Value) -> f64 {
    let la = leaves(a);
    let lb = leaves(b);
    if la.is_empty() && lb.is_empty() {
        return 1.0;
    }
    let inter = la.intersection(&lb).count() as f64;
    let union = la.union(&lb).count() as f64;
    inter / union
}

fn leaves(value: &Value) -> HashSet<String> {
    let mut out = HashSet::new();
    collect(String::new(), value, &mut out);
    out
}

fn collect(path: String, value: &Value, out: &mut HashSet<String>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (k, v) in map {
                let child = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                collect(child, v, out);
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (i, v) in items.iter().enumerate() {
                collect(format!("{path}[{i}]"), v, out);
            }
        }
        other => {
            out.insert(format!("{path}={other}"));
        }
    }
}

/// Pairs `(removed index, added index)` for resources judged to be renames.
/// Each resource is claimed at most once; the best-scoring candidate wins.
pub fn detect_renames(removed: &[&Resource], added: &[&Resource]) -> Vec<(usize, usize)> {
    let mut candidates: Vec<(f64, usize, usize)> = Vec::new();
    for (ri, r) in removed.iter().enumerate() {
        for (ai, a) in added.iter().enumerate() {
            if r.kind != a.kind {
                continue;
            }
            let score = similarity(&r.properties, &a.properties);
            if score >= RENAME_SIMILARITY_THRESHOLD {
                candidates.push((score, ri, ai));
            }
        }
    }
    candidates.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut used_removed = HashSet::new();
    let mut used_added = HashSet::new();
    let mut pairs = Vec::new();
    for (_, ri, ai) in candidates {
        if used_removed.contains(&ri) || used_added.contains(&ai) {
            continue;
        }
        used_removed.insert(ri);
        used_added.insert(ai);
        pairs.push((ri, ai));
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DocKind;
    use serde_json::json;

    fn res(id: &str, kind: &str, props: Value) -> Resource {
        Resource {
            id: id.into(),
            kind: kind.into(),
            properties: props,
            source_kind: DocKind::CloudFormation,
            forces_replacement: false,
        }
    }

    #[test]
    fn identical_properties_score_one() {
        let v = json!({"a": 1, "b": {"c": [1, 2]}});
        assert_eq!(similarity(&v, &v), 1.0);
    }

    #[test]
    fn disjoint_properties_score_zero() {
        assert_eq!(similarity(&json!({"a": 1}), &json!({"b": 2})), 0.0);
    }

    #[test]
    fn similarity_is_jaccard_over_leaves() {
        // 9 shared leaves + 1 differing leaf on each side → 9 / 11.
        let mut a = serde_json::Map::new();
        let mut b = serde_json::Map::new();
        for i in 0..9 {
            a.insert(format!("k{i}"), json!(i));
            b.insert(format!("k{i}"), json!(i));
        }
        a.insert("x".into(), json!("old"));
        b.insert("x".into(), json!("new"));
        let s = similarity(&Value::Object(a), &Value::Object(b));
        assert!((s - 9.0 / 11.0).abs() < 1e-9, "{s}");
    }

    #[test]
    fn empty_objects_are_fully_similar() {
        assert_eq!(similarity(&json!({}), &json!({})), 1.0);
        assert!(similarity(&json!({}), &json!({"a": 1})) < 1.0);
    }

    #[test]
    fn rename_detected_above_threshold() {
        let props = json!({"BucketName": "assets", "VersioningConfiguration": {"Status": "Enabled"},
            "Tags": [{"Key": "team", "Value": "web"}]});
        let removed = res("Assets", "AWS::S3::Bucket", props.clone());
        let added = res("StaticAssets", "AWS::S3::Bucket", props);
        assert_eq!(detect_renames(&[&removed], &[&added]), vec![(0, 0)]);
    }

    #[test]
    fn different_types_never_pair() {
        let removed = res("A", "AWS::S3::Bucket", json!({"x": 1}));
        let added = res("B", "AWS::SQS::Queue", json!({"x": 1}));
        assert!(detect_renames(&[&removed], &[&added]).is_empty());
    }

    #[test]
    fn dissimilar_properties_do_not_pair() {
        let removed = res(
            "A",
            "aws_instance",
            json!({"instance_type": "t3.micro", "ami": "ami-1"}),
        );
        let added = res(
            "B",
            "aws_instance",
            json!({"instance_type": "m5.large", "ami": "ami-2"}),
        );
        assert!(detect_renames(&[&removed], &[&added]).is_empty());
    }

    #[test]
    fn best_candidate_wins_and_each_is_claimed_once() {
        let base = |n: &str| {
            let mut m = serde_json::Map::new();
            for i in 0..20 {
                m.insert(format!("k{i}"), json!(i));
            }
            m.insert("name".into(), json!(n));
            Value::Object(m)
        };
        let removed_a = res("old_a", "aws_sqs_queue", base("a"));
        let removed_b = res("old_b", "aws_sqs_queue", base("b"));
        let added_a = res("new_a", "aws_sqs_queue", base("a"));
        let added_b = res("new_b", "aws_sqs_queue", base("b"));
        let mut pairs = detect_renames(&[&removed_a, &removed_b], &[&added_b, &added_a]);
        pairs.sort();
        assert_eq!(pairs, vec![(0, 1), (1, 0)]);
    }
}
