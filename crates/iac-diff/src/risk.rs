//! Rule-based risk tagging. Each rule contributes `(Risk, reason)` findings;
//! the resource takes the highest and keeps every reason.

use crate::model::{ChangeOp, Op, PropertyChange, ResourceDiff, Risk};
use serde_json::Value;

pub struct Assessment {
    pub risk: Risk,
    pub reasons: Vec<String>,
}

/// Resources that hold data. Destroying one is never a rollback-able change.
const STATEFUL_KINDS: &[&str] = &[
    "AWS::RDS::DBInstance",
    "AWS::RDS::DBCluster",
    "AWS::DynamoDB::Table",
    "AWS::DynamoDB::GlobalTable",
    "AWS::S3::Bucket",
    "AWS::EC2::Volume",
    "AWS::EFS::FileSystem",
    "AWS::FSx::FileSystem",
    "AWS::Kinesis::Stream",
    "AWS::SQS::Queue",
    "AWS::Redshift::Cluster",
    "AWS::DocDB::DBCluster",
    "AWS::Neptune::DBCluster",
    "AWS::SecretsManager::Secret",
    "AWS::KMS::Key",
    "AWS::Elasticsearch::Domain",
    "AWS::OpenSearchService::Domain",
    "AWS::Logs::LogGroup",
    "AWS::Backup::BackupVault",
    "AWS::MSK::Cluster",
    "AWS::MemoryDB::Cluster",
    "AWS::QLDB::Ledger",
    "AWS::Cognito::UserPool",
    "aws_db_instance",
    "aws_rds_cluster",
    "aws_dynamodb_table",
    "aws_s3_bucket",
    "aws_ebs_volume",
    "aws_efs_file_system",
    "aws_kinesis_stream",
    "aws_sqs_queue",
    "aws_redshift_cluster",
    "aws_docdb_cluster",
    "aws_neptune_cluster",
    "aws_secretsmanager_secret",
    "aws_kms_key",
    "aws_opensearch_domain",
    "aws_elasticsearch_domain",
    "aws_cloudwatch_log_group",
    "aws_backup_vault",
    "aws_msk_cluster",
    "aws_memorydb_cluster",
    "aws_cognito_user_pool",
    "azurerm_storage_account",
    "azurerm_mssql_database",
    "azurerm_mssql_server",
    "azurerm_postgresql_flexible_server",
    "azurerm_mysql_flexible_server",
    "azurerm_cosmosdb_account",
    "azurerm_key_vault",
    "azurerm_managed_disk",
    "google_sql_database_instance",
    "google_storage_bucket",
    "google_bigtable_instance",
    "google_spanner_instance",
    "google_kms_crypto_key",
];

const STATEFUL_PREFIXES: &[&str] = &[
    "AWS::ElastiCache::",
    "AWS::Timestream::",
    "aws_elasticache_",
    "aws_fsx_",
];

/// Guardrails: removing one weakens the posture of something else.
const PROTECTIVE_KINDS: &[&str] = &[
    "aws_s3_bucket_public_access_block",
    "aws_s3_account_public_access_block",
    "aws_s3_bucket_server_side_encryption_configuration",
    "aws_cloudtrail",
    "AWS::CloudTrail::Trail",
    "aws_guardduty_detector",
    "AWS::GuardDuty::Detector",
    "aws_config_configuration_recorder",
    "AWS::Config::ConfigurationRecorder",
    "aws_wafv2_web_acl",
    "AWS::WAFv2::WebACL",
    "aws_backup_plan",
    "AWS::Backup::BackupPlan",
];

const INGRESS_KINDS: &[&str] = &[
    "AWS::EC2::SecurityGroupIngress",
    "aws_vpc_security_group_ingress_rule",
];

const ENCRYPTION_BOOL_LEAVES: &[&str] = &[
    "StorageEncrypted",
    "storage_encrypted",
    "Encrypted",
    "encrypted",
    "SSEEnabled",
    "sse_enabled",
    "EncryptionAtRest",
    "encryption_at_rest",
    "KmsKeyEnabled",
    "at_rest_encryption_enabled",
    "transit_encryption_enabled",
    "TransitEncryptionEnabled",
    "AtRestEncryptionEnabled",
    "EnableKeyRotation",
    "enable_key_rotation",
];

const KMS_LEAVES: &[&str] = &[
    "KmsKeyId",
    "kms_key_id",
    "KmsKeyArn",
    "kms_key_arn",
    "KmsMasterKeyId",
    "kms_master_key_id",
    "SSEKMSKeyId",
    "sse_kms_key_id",
    "KMSMasterKeyID",
    "kms_key",
    "PerformanceInsightsKMSKeyId",
    "performance_insights_kms_key_id",
];

const ENCRYPTION_BLOCK_LEAVES: &[&str] = &[
    "BucketEncryption",
    "ServerSideEncryptionConfiguration",
    "server_side_encryption_configuration",
    "EncryptionConfiguration",
    "encryption_configuration",
    "SSESpecification",
    "server_side_encryption",
    "EncryptionAtRestOptions",
    "encrypt_at_rest",
    "NodeToNodeEncryptionOptions",
    "node_to_node_encryption",
];

const PUBLIC_TRUE_LEAVES: &[&str] = &[
    "PubliclyAccessible",
    "publicly_accessible",
    "MapPublicIpOnLaunch",
    "map_public_ip_on_launch",
    "AssociatePublicIpAddress",
    "associate_public_ip_address",
    "public_network_access_enabled",
    "PublicNetworkAccess",
    "allow_blob_public_access",
    "AllowBlobPublicAccess",
    "AssignPublicIp",
    "assign_public_ip",
];

const PUBLIC_BLOCK_LEAVES: &[&str] = &[
    "BlockPublicAcls",
    "BlockPublicPolicy",
    "IgnorePublicAcls",
    "RestrictPublicBuckets",
    "block_public_acls",
    "block_public_policy",
    "ignore_public_acls",
    "restrict_public_buckets",
];

const PUBLIC_ACL_LEAVES: &[&str] = &["AccessControl", "acl"];
const PUBLIC_ACLS: &[&str] = &[
    "PublicRead",
    "PublicReadWrite",
    "AuthenticatedRead",
    "public-read",
    "public-read-write",
    "authenticated-read",
];

const SIZE_LEAVES: &[&str] = &[
    "InstanceType",
    "instance_type",
    "DBInstanceClass",
    "instance_class",
    "CacheNodeType",
    "cache_node_type",
    "node_type",
    "NodeType",
    "InstanceClass",
    "AllocatedStorage",
    "allocated_storage",
    "size",
    "vm_size",
    "machine_type",
    "ClusterInstanceType",
    "ComputeType",
    "compute_type",
    "MemorySize",
    "memory_size",
];

const TAG_ROOTS: &[&str] = &["Tags", "tags", "tags_all", "TagSpecifications"];

pub fn is_stateful(kind: &str) -> bool {
    STATEFUL_KINDS.contains(&kind) || STATEFUL_PREFIXES.iter().any(|p| kind.starts_with(p))
}

pub fn is_protective(kind: &str) -> bool {
    PROTECTIVE_KINDS.contains(&kind)
}

pub fn assess(diff: &ResourceDiff) -> Assessment {
    let mut findings: Vec<(Risk, String)> = Vec::new();
    match diff.op {
        Op::Unchanged => {
            return Assessment {
                risk: Risk::Low,
                reasons: Vec::new(),
            }
        }
        Op::Removed => {
            if is_stateful(&diff.kind) {
                findings.push((
                    Risk::Critical,
                    format!(
                        "Stateful resource removed — {} data will be destroyed",
                        diff.kind
                    ),
                ));
            } else if is_protective(&diff.kind) {
                findings.push((
                    Risk::High,
                    format!(
                        "Guardrail removed — {} protected other resources",
                        diff.kind
                    ),
                ));
            } else {
                findings.push((Risk::Medium, "Resource removed".to_string()));
            }
        }
        Op::Added => {
            let leaves = leaf_changes(diff.after.as_ref().unwrap_or(&Value::Null));
            evaluate_changes(diff, &leaves, &mut findings);
            if findings.is_empty() {
                findings.push((Risk::Low, "New resource".to_string()));
            }
        }
        Op::Modified => {
            if diff.forces_replacement {
                if is_stateful(&diff.kind) {
                    findings.push((
                        Risk::Critical,
                        "Plan replaces the resource (destroy + create) — stateful, its data is lost"
                            .to_string(),
                    ));
                } else {
                    findings.push((
                        Risk::Medium,
                        "Plan replaces the resource (destroy + create)".to_string(),
                    ));
                }
            }
            evaluate_changes(diff, &diff.changes, &mut findings);
            if findings.is_empty() {
                if tag_only(&diff.changes) {
                    findings.push((Risk::Low, "Tag-only change".to_string()));
                } else {
                    findings.push((Risk::Medium, "Properties modified".to_string()));
                }
            }
        }
        Op::Renamed => {
            let from = diff.renamed_from.as_deref().unwrap_or("?");
            if is_stateful(&diff.kind) {
                findings.push((
                    Risk::High,
                    format!(
                        "Renamed from {from} — a stateful resource is replaced on rename unless \
                         migrated (Terraform `moved` block / CloudFormation import)"
                    ),
                ));
            } else {
                findings.push((Risk::Low, format!("Renamed from {from}")));
            }
            evaluate_changes(diff, &diff.changes, &mut findings);
        }
    }
    findings.sort_by(|a, b| b.0.cmp(&a.0));
    let risk = findings.first().map(|f| f.0).unwrap_or(Risk::Low);
    let mut reasons: Vec<String> = Vec::new();
    for (_, reason) in findings {
        if !reasons.contains(&reason) {
            reasons.push(reason);
        }
    }
    Assessment { risk, reasons }
}

fn evaluate_changes(
    diff: &ResourceDiff,
    changes: &[PropertyChange],
    out: &mut Vec<(Risk, String)>,
) {
    for change in changes {
        let leaf = change.leaf();
        policy_weakened(change, leaf, out);
        ingress_widened(diff, change, leaf, out);
        iam_wildcard(diff, change, leaf, out);
        encryption_weakened(diff, change, leaf, out);
        public_access(change, leaf, out);
        size_changed(change, leaf, out);
    }
}

// ------------------------------------------------------------------ rules

fn policy_rank(value: Option<&Value>) -> Option<u8> {
    match value {
        None => Some(0),
        Some(Value::String(s)) => match s.as_str() {
            "Retain" | "RetainExceptOnCreate" => Some(2),
            "Snapshot" => Some(1),
            "Delete" => Some(0),
            _ => None,
        },
        Some(_) => None,
    }
}

fn policy_weakened(change: &PropertyChange, leaf: &str, out: &mut Vec<(Risk, String)>) {
    if leaf != "DeletionPolicy" && leaf != "UpdateReplacePolicy" {
        return;
    }
    if let (Some(b), Some(a)) = (
        policy_rank(change.before.as_ref()),
        policy_rank(change.after.as_ref()),
    ) {
        if a < b {
            out.push((
                Risk::High,
                format!(
                    "{leaf} weakened: {} → {}",
                    fmt(change.before.as_ref()),
                    fmt(change.after.as_ref())
                ),
            ));
        }
    }
}

fn is_world_cidr(s: &str) -> bool {
    matches!(s, "0.0.0.0/0" | "::/0")
}

fn contains_world_cidr(value: Option<&Value>) -> bool {
    match value {
        Some(Value::String(s)) => is_world_cidr(s),
        Some(Value::Array(items)) => items.iter().any(|v| contains_world_cidr(Some(v))),
        Some(Value::Object(map)) => map.values().any(|v| contains_world_cidr(Some(v))),
        _ => false,
    }
}

fn in_ingress_context(diff: &ResourceDiff, path: &str) -> bool {
    if INGRESS_KINDS.contains(&diff.kind.as_str()) {
        return true;
    }
    if diff.kind == "aws_security_group_rule" {
        let side = diff.after.as_ref().or(diff.before.as_ref());
        return side
            .and_then(|v| v.get("type"))
            .and_then(Value::as_str)
            .map(|t| t == "ingress")
            .unwrap_or(false);
    }
    path.split(['.', '[']).any(|seg| {
        let s = seg.trim_end_matches(']');
        s == "ingress" || s == "Ingress" || s == "SecurityGroupIngress"
    })
}

fn ingress_widened(
    diff: &ResourceDiff,
    change: &PropertyChange,
    leaf: &str,
    out: &mut Vec<(Risk, String)>,
) {
    if !in_ingress_context(diff, &change.path) {
        return;
    }
    if contains_world_cidr(change.after.as_ref()) && !contains_world_cidr(change.before.as_ref()) {
        let shown = match change.after.as_ref() {
            Some(Value::String(s)) => format!("{} = {s}", change.path),
            _ => format!("{} includes 0.0.0.0/0 or ::/0", change.path),
        };
        out.push((
            Risk::Critical,
            format!("Ingress opened to the world: {shown}"),
        ));
    }
    if change.op == ChangeOp::Modified {
        let (b, a) = (change.before.as_ref(), change.after.as_ref());
        let broadened = match leaf {
            "FromPort" | "from_port" => num(b).zip(num(a)).map(|(b, a)| a < b),
            "ToPort" | "to_port" => num(b).zip(num(a)).map(|(b, a)| a > b),
            _ => None,
        };
        if broadened == Some(true) {
            out.push((
                Risk::Critical,
                format!(
                    "Ingress port range broadened: {leaf} {} → {}",
                    fmt(b),
                    fmt(a)
                ),
            ));
        }
        if matches!(leaf, "IpProtocol" | "protocol" | "ip_protocol")
            && is_all_protocols(a)
            && !is_all_protocols(b)
        {
            out.push((
                Risk::Critical,
                format!(
                    "Ingress protocol widened to all traffic: {leaf} {} → {}",
                    fmt(b),
                    fmt(a)
                ),
            ));
        }
    }
}

fn is_all_protocols(v: Option<&Value>) -> bool {
    match v {
        Some(Value::String(s)) => s == "-1" || s.eq_ignore_ascii_case("all"),
        Some(Value::Number(n)) => n.as_i64() == Some(-1),
        _ => false,
    }
}

fn num(v: Option<&Value>) -> Option<f64> {
    match v {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

fn in_iam_context(diff: &ResourceDiff, path: &str) -> bool {
    let k = diff.kind.to_ascii_lowercase();
    path.contains("Statement") || k.contains("iam") || k.contains("policy")
}

fn wildcard_actions(value: Option<&Value>) -> Vec<String> {
    let mut found = Vec::new();
    collect_wildcards(value, &mut found, |s| s == "*" || s.ends_with(":*"));
    found
}

fn wildcard_resources(value: Option<&Value>) -> Vec<String> {
    let mut found = Vec::new();
    collect_wildcards(value, &mut found, |s| s == "*");
    found
}

fn collect_wildcards(value: Option<&Value>, out: &mut Vec<String>, pred: fn(&str) -> bool) {
    match value {
        Some(Value::String(s)) if pred(s) => out.push(s.clone()),
        Some(Value::Array(items)) => items
            .iter()
            .for_each(|v| collect_wildcards(Some(v), out, pred)),
        Some(Value::Object(map)) => map
            .values()
            .for_each(|v| collect_wildcards(Some(v), out, pred)),
        _ => {}
    }
}

fn iam_wildcard(
    diff: &ResourceDiff,
    change: &PropertyChange,
    leaf: &str,
    out: &mut Vec<(Risk, String)>,
) {
    if !in_iam_context(diff, &change.path) {
        return;
    }
    let (b, a) = (change.before.as_ref(), change.after.as_ref());
    match leaf {
        "Action" | "NotResource" => {
            let before = wildcard_actions(b);
            for w in wildcard_actions(a) {
                if !before.contains(&w) {
                    out.push((
                        Risk::High,
                        format!("IAM policy grants wildcard action: {w}"),
                    ));
                }
            }
        }
        "Resource" => {
            let before = wildcard_resources(b);
            for w in wildcard_resources(a) {
                if !before.contains(&w) {
                    out.push((
                        Risk::High,
                        format!("IAM policy applies to every resource: {w}"),
                    ));
                }
            }
        }
        "Statement" | "PolicyDocument" | "policy" | "inline_policy" | "assume_role_policy" => {
            // A whole statement added or replaced: look inside it.
            let before_actions = wildcard_actions(b);
            let before_resources = wildcard_resources(b);
            let mut actions = Vec::new();
            let mut resources = Vec::new();
            collect_by_key(a, "Action", &mut actions);
            collect_by_key(a, "Resource", &mut resources);
            for w in wildcard_actions(Some(&Value::Array(actions))) {
                if !before_actions.contains(&w) {
                    out.push((
                        Risk::High,
                        format!("IAM policy grants wildcard action: {w}"),
                    ));
                }
            }
            for w in wildcard_resources(Some(&Value::Array(resources))) {
                if !before_resources.contains(&w) {
                    out.push((
                        Risk::High,
                        format!("IAM policy applies to every resource: {w}"),
                    ));
                }
            }
        }
        _ => {}
    }
}

fn collect_by_key(value: Option<&Value>, key: &str, out: &mut Vec<Value>) {
    match value {
        Some(Value::Object(map)) => {
            for (k, v) in map {
                if k == key {
                    out.push(v.clone());
                } else {
                    collect_by_key(Some(v), key, out);
                }
            }
        }
        Some(Value::Array(items)) => items.iter().for_each(|v| collect_by_key(Some(v), key, out)),
        _ => {}
    }
}

fn is_true(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Bool(true))) || matches!(v, Some(Value::String(s)) if s == "true")
}

fn is_false_or_absent(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => true,
        Some(Value::String(s)) => s == "false",
        _ => false,
    }
}

fn is_empty_or_absent(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.is_empty(),
        _ => false,
    }
}

fn encryption_weakened(
    diff: &ResourceDiff,
    change: &PropertyChange,
    leaf: &str,
    out: &mut Vec<(Risk, String)>,
) {
    let (b, a) = (change.before.as_ref(), change.after.as_ref());
    let lower_path = change.path.to_ascii_lowercase();
    let encryption_flag = ENCRYPTION_BOOL_LEAVES.contains(&leaf)
        || ((leaf == "Enabled" || leaf == "enabled")
            && (lower_path.contains("encrypt") || lower_path.contains("sse")));
    if encryption_flag {
        if is_true(b) && is_false_or_absent(a) {
            out.push((
                Risk::High,
                format!("Encryption disabled: {} true → {}", change.path, fmt(a)),
            ));
        } else if diff.op == Op::Added
            && matches!(a, Some(Value::Bool(false)))
            && is_stateful(&diff.kind)
        {
            out.push((
                Risk::Medium,
                format!("Created with encryption disabled: {}", change.path),
            ));
        }
    }
    if KMS_LEAVES.contains(&leaf) && !is_empty_or_absent(b) && is_empty_or_absent(a) {
        out.push((Risk::High, format!("KMS key removed: {}", change.path)));
    }
    if ENCRYPTION_BLOCK_LEAVES.contains(&leaf) && change.op == ChangeOp::Removed {
        out.push((
            Risk::High,
            format!("Encryption configuration removed: {}", change.path),
        ));
    }
    if matches!(leaf, "SSEAlgorithm" | "sse_algorithm") {
        let was_kms = matches!(b, Some(Value::String(s)) if s.contains("kms"));
        let now_kms = matches!(a, Some(Value::String(s)) if s.contains("kms"));
        if was_kms && !now_kms && a.is_some() {
            out.push((
                Risk::Medium,
                format!("Encryption downgraded from KMS: {} → {}", fmt(b), fmt(a)),
            ));
        }
    }
}

fn public_access(change: &PropertyChange, leaf: &str, out: &mut Vec<(Risk, String)>) {
    let (b, a) = (change.before.as_ref(), change.after.as_ref());
    if PUBLIC_TRUE_LEAVES.contains(&leaf) && is_true(a) && !is_true(b) {
        out.push((
            Risk::High,
            format!("Public access enabled: {} = true", change.path),
        ));
    }
    if PUBLIC_BLOCK_LEAVES.contains(&leaf) && is_true(b) && !is_true(a) {
        out.push((
            Risk::High,
            format!(
                "Public access block weakened: {} true → {}",
                change.path,
                fmt(a)
            ),
        ));
    }
    if matches!(
        leaf,
        "PublicAccessBlockConfiguration" | "public_access_block_configuration"
    ) && change.op == ChangeOp::Removed
    {
        out.push((
            Risk::High,
            format!("Public access block removed: {}", change.path),
        ));
    }
    if PUBLIC_ACL_LEAVES.contains(&leaf) {
        let now_public = matches!(a, Some(Value::String(s)) if PUBLIC_ACLS.contains(&s.as_str()));
        let was_public = matches!(b, Some(Value::String(s)) if PUBLIC_ACLS.contains(&s.as_str()));
        if now_public && !was_public {
            out.push((
                Risk::High,
                format!("Public ACL applied: {} = {}", change.path, fmt(a)),
            ));
        }
    }
}

fn size_changed(change: &PropertyChange, leaf: &str, out: &mut Vec<(Risk, String)>) {
    if change.op == ChangeOp::Modified && SIZE_LEAVES.contains(&leaf) {
        out.push((
            Risk::Medium,
            format!(
                "Instance class / size changed: {} {} → {}",
                change.path,
                fmt(change.before.as_ref()),
                fmt(change.after.as_ref())
            ),
        ));
    }
}

fn tag_only(changes: &[PropertyChange]) -> bool {
    !changes.is_empty()
        && changes.iter().all(|c| {
            let root = c.path.split(['.', '[']).next().unwrap_or("");
            TAG_ROOTS.contains(&root)
        })
}

/// Every leaf of a freshly added resource, expressed as an `Added` change so
/// the same rules that read modifications can read a new resource.
fn leaf_changes(value: &Value) -> Vec<PropertyChange> {
    let mut out = Vec::new();
    flatten(String::new(), value, &mut out);
    out
}

fn flatten(path: String, value: &Value, out: &mut Vec<PropertyChange>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (k, v) in map {
                let child = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                flatten(child, v, out);
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (i, v) in items.iter().enumerate() {
                flatten(format!("{path}[{i}]"), v, out);
            }
        }
        _ => out.push(PropertyChange {
            path,
            before_path: None,
            op: ChangeOp::Added,
            before: None,
            after: Some(value.clone()),
        }),
    }
}

fn fmt(value: Option<&Value>) -> String {
    match value {
        None => "(absent)".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) => "null".to_string(),
        Some(other) => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::propdiff::diff_values;
    use serde_json::json;

    fn modified(kind: &str, before: Value, after: Value) -> ResourceDiff {
        ResourceDiff {
            id: "r".into(),
            renamed_from: None,
            kind: kind.into(),
            op: Op::Modified,
            risk: Risk::Low,
            reasons: vec![],
            changes: diff_values(&before, &after),
            before: Some(before),
            after: Some(after),
            forces_replacement: false,
        }
    }

    fn with_op(kind: &str, op: Op, value: Value) -> ResourceDiff {
        let (before, after) = match op {
            Op::Removed => (Some(value), None),
            _ => (None, Some(value)),
        };
        ResourceDiff {
            id: "r".into(),
            renamed_from: None,
            kind: kind.into(),
            op,
            risk: Risk::Low,
            reasons: vec![],
            changes: vec![],
            before,
            after,
            forces_replacement: false,
        }
    }

    fn has(a: &Assessment, needle: &str) -> bool {
        a.reasons.iter().any(|r| r.contains(needle))
    }

    #[test]
    fn removed_stateful_is_critical() {
        for kind in [
            "AWS::RDS::DBInstance",
            "aws_s3_bucket",
            "AWS::ElastiCache::ReplicationGroup",
            "aws_elasticache_cluster",
        ] {
            let a = assess(&with_op(kind, Op::Removed, json!({})));
            assert_eq!(a.risk, Risk::Critical, "{kind}");
            assert!(has(&a, "destroyed"));
        }
    }

    #[test]
    fn removed_guardrail_is_high() {
        let a = assess(&with_op(
            "aws_s3_bucket_public_access_block",
            Op::Removed,
            json!({}),
        ));
        assert_eq!(a.risk, Risk::High);
        assert!(has(&a, "Guardrail"));
    }

    #[test]
    fn removed_stateless_is_medium() {
        let a = assess(&with_op("AWS::SNS::Topic", Op::Removed, json!({})));
        assert_eq!(a.risk, Risk::Medium);
    }

    #[test]
    fn added_plain_resource_is_low() {
        let a = assess(&with_op(
            "aws_cloudwatch_log_group",
            Op::Added,
            json!({"name": "x", "retention_in_days": 14}),
        ));
        assert_eq!(a.risk, Risk::Low);
        assert_eq!(a.reasons, vec!["New resource"]);
    }

    #[test]
    fn unchanged_has_no_reasons() {
        let a = assess(&with_op("aws_vpc", Op::Unchanged, json!({})));
        assert_eq!(a.risk, Risk::Low);
        assert!(a.reasons.is_empty());
    }

    #[test]
    fn deletion_policy_weakened_is_high() {
        let a = assess(&modified(
            "AWS::DynamoDB::Table",
            json!({"DeletionPolicy": "Retain"}),
            json!({"DeletionPolicy": "Delete"}),
        ));
        assert_eq!(a.risk, Risk::High);
        assert!(has(&a, "DeletionPolicy weakened: Retain → Delete"));
        let removed = assess(&modified(
            "AWS::DynamoDB::Table",
            json!({"UpdateReplacePolicy": "Snapshot", "x": 1}),
            json!({"x": 1}),
        ));
        assert_eq!(removed.risk, Risk::High);
        assert!(has(
            &removed,
            "UpdateReplacePolicy weakened: Snapshot → (absent)"
        ));
    }

    #[test]
    fn deletion_policy_strengthened_is_not_flagged() {
        let a = assess(&modified(
            "AWS::S3::Bucket",
            json!({"DeletionPolicy": "Delete"}),
            json!({"DeletionPolicy": "Retain"}),
        ));
        assert_eq!(a.risk, Risk::Medium);
        assert!(!has(&a, "weakened"));
    }

    #[test]
    fn ingress_opened_to_world_is_critical_cfn() {
        let a = assess(&modified(
            "AWS::EC2::SecurityGroup",
            json!({"SecurityGroupIngress": [{"IpProtocol": "tcp", "FromPort": 22, "ToPort": 22, "CidrIp": "10.0.0.0/8"}]}),
            json!({"SecurityGroupIngress": [{"IpProtocol": "tcp", "FromPort": 22, "ToPort": 22, "CidrIp": "0.0.0.0/0"}]}),
        ));
        assert_eq!(a.risk, Risk::Critical);
        assert!(has(&a, "Ingress opened to the world"));
    }

    #[test]
    fn ingress_opened_to_world_is_critical_tf_cidr_added() {
        let a = assess(&modified(
            "aws_security_group",
            json!({"ingress": [{"from_port": 443, "to_port": 443, "cidr_blocks": ["10.0.0.0/8"]}]}),
            json!({"ingress": [{"from_port": 443, "to_port": 443, "cidr_blocks": ["10.0.0.0/8", "::/0"]}]}),
        ));
        assert_eq!(a.risk, Risk::Critical);
    }

    #[test]
    fn egress_to_world_is_not_ingress() {
        let a = assess(&modified(
            "aws_security_group",
            json!({"egress": [{"cidr_blocks": ["10.0.0.0/8"]}]}),
            json!({"egress": [{"cidr_blocks": ["0.0.0.0/0"]}]}),
        ));
        assert_eq!(a.risk, Risk::Medium);
    }

    #[test]
    fn ingress_port_range_broadened_is_critical() {
        let a = assess(&modified(
            "AWS::EC2::SecurityGroupIngress",
            json!({"FromPort": 443, "ToPort": 443, "CidrIp": "10.0.0.0/8"}),
            json!({"FromPort": 0, "ToPort": 65535, "CidrIp": "10.0.0.0/8"}),
        ));
        assert_eq!(a.risk, Risk::Critical);
        assert!(has(&a, "FromPort 443 → 0"));
        assert!(has(&a, "ToPort 443 → 65535"));
    }

    #[test]
    fn ingress_port_range_narrowed_is_fine() {
        let a = assess(&modified(
            "aws_security_group_rule",
            json!({"type": "ingress", "from_port": 0, "to_port": 65535}),
            json!({"type": "ingress", "from_port": 443, "to_port": 443}),
        ));
        assert_eq!(a.risk, Risk::Medium);
    }

    #[test]
    fn ingress_protocol_all_is_critical() {
        let a = assess(&modified(
            "aws_security_group_rule",
            json!({"type": "ingress", "protocol": "tcp"}),
            json!({"type": "ingress", "protocol": "-1"}),
        ));
        assert_eq!(a.risk, Risk::Critical);
    }

    #[test]
    fn added_security_group_open_to_world_is_critical() {
        let a = assess(&with_op(
            "aws_security_group",
            Op::Added,
            json!({"name": "web", "ingress": [{"from_port": 80, "to_port": 80, "cidr_blocks": ["0.0.0.0/0"]}]}),
        ));
        assert_eq!(a.risk, Risk::Critical);
    }

    #[test]
    fn iam_wildcard_action_is_high() {
        let a = assess(&modified(
            "AWS::IAM::Role",
            json!({"Policies": [{"PolicyDocument": {"Statement": [{"Effect": "Allow", "Action": ["s3:GetObject"], "Resource": "arn:aws:s3:::b/*"}]}}]}),
            json!({"Policies": [{"PolicyDocument": {"Statement": [{"Effect": "Allow", "Action": ["s3:GetObject", "s3:*"], "Resource": "arn:aws:s3:::b/*"}]}}]}),
        ));
        assert_eq!(a.risk, Risk::High);
        assert!(has(&a, "wildcard action: s3:*"));
    }

    #[test]
    fn iam_wildcard_resource_is_high_and_arn_glob_is_not() {
        let a = assess(&modified(
            "aws_iam_policy",
            json!({"policy": {"Statement": [{"Action": "s3:GetObject", "Resource": "arn:aws:s3:::b/*"}]}}),
            json!({"policy": {"Statement": [{"Action": "s3:GetObject", "Resource": "*"}]}}),
        ));
        assert_eq!(a.risk, Risk::High);
        assert!(has(&a, "every resource: *"));
    }

    #[test]
    fn iam_new_statement_with_wildcard_is_high() {
        let a = assess(&modified(
            "aws_iam_role",
            json!({"inline_policy": [{"policy": {"Statement": [{"Action": "logs:PutLogEvents", "Resource": "*"}]}}]}),
            json!({"inline_policy": [{"policy": {"Statement": [{"Action": "logs:PutLogEvents", "Resource": "*"}, {"Action": "*", "Resource": "*"}]}}]}),
        ));
        assert_eq!(a.risk, Risk::High);
        assert!(has(&a, "wildcard action: *"));
    }

    #[test]
    fn iam_wildcard_already_present_is_not_re_flagged() {
        let a = assess(&modified(
            "aws_iam_policy",
            json!({"policy": {"Statement": [{"Action": "*", "Resource": "*"}]}, "description": "a"}),
            json!({"policy": {"Statement": [{"Action": "*", "Resource": "*"}]}, "description": "b"}),
        ));
        assert_eq!(a.risk, Risk::Medium);
    }

    #[test]
    fn encryption_disabled_is_high() {
        let a = assess(&modified(
            "aws_db_instance",
            json!({"storage_encrypted": true}),
            json!({"storage_encrypted": false}),
        ));
        assert_eq!(a.risk, Risk::High);
        assert!(has(&a, "Encryption disabled"));
        let nested = assess(&modified(
            "AWS::DynamoDB::Table",
            json!({"SSESpecification": {"SSEEnabled": true}}),
            json!({"SSESpecification": {"SSEEnabled": false}}),
        ));
        assert_eq!(nested.risk, Risk::High);
    }

    #[test]
    fn kms_key_removed_is_high() {
        let a = assess(&modified(
            "aws_sqs_queue",
            json!({"kms_master_key_id": "alias/x", "name": "q"}),
            json!({"name": "q"}),
        ));
        assert_eq!(a.risk, Risk::High);
        assert!(has(&a, "KMS key removed"));
        let blanked = assess(&modified(
            "AWS::RDS::DBInstance",
            json!({"KmsKeyId": "k"}),
            json!({"KmsKeyId": ""}),
        ));
        assert_eq!(blanked.risk, Risk::High);
    }

    #[test]
    fn encryption_block_removed_is_high() {
        let a = assess(&modified(
            "AWS::S3::Bucket",
            json!({"BucketEncryption": {"ServerSideEncryptionConfiguration": []}, "BucketName": "b"}),
            json!({"BucketName": "b"}),
        ));
        assert_eq!(a.risk, Risk::High);
    }

    #[test]
    fn sse_downgrade_from_kms_is_medium() {
        let a = assess(&modified(
            "AWS::S3::Bucket",
            json!({"BucketEncryption": {"ServerSideEncryptionConfiguration": [{"ServerSideEncryptionByDefault": {"SSEAlgorithm": "aws:kms"}}]}}),
            json!({"BucketEncryption": {"ServerSideEncryptionConfiguration": [{"ServerSideEncryptionByDefault": {"SSEAlgorithm": "AES256"}}]}}),
        ));
        assert_eq!(a.risk, Risk::Medium);
        assert!(has(&a, "downgraded from KMS"));
    }

    #[test]
    fn publicly_accessible_enabled_is_high() {
        let a = assess(&modified(
            "aws_db_instance",
            json!({"publicly_accessible": false}),
            json!({"publicly_accessible": true}),
        ));
        assert_eq!(a.risk, Risk::High);
        assert!(has(&a, "Public access enabled"));
        let added = assess(&with_op(
            "AWS::RDS::DBInstance",
            Op::Added,
            json!({"PubliclyAccessible": true}),
        ));
        assert_eq!(added.risk, Risk::High);
    }

    #[test]
    fn public_access_block_weakened_is_high() {
        let a = assess(&modified(
            "aws_s3_bucket_public_access_block",
            json!({"block_public_acls": true, "block_public_policy": true}),
            json!({"block_public_acls": false, "block_public_policy": true}),
        ));
        assert_eq!(a.risk, Risk::High);
        let removed = assess(&modified(
            "AWS::S3::Bucket",
            json!({"PublicAccessBlockConfiguration": {"BlockPublicAcls": true}}),
            json!({}),
        ));
        assert_eq!(removed.risk, Risk::High);
        assert!(has(&removed, "Public access block removed"));
    }

    #[test]
    fn public_acl_is_high() {
        let a = assess(&modified(
            "AWS::S3::Bucket",
            json!({"AccessControl": "Private"}),
            json!({"AccessControl": "PublicRead"}),
        ));
        assert_eq!(a.risk, Risk::High);
        let tf = assess(&modified(
            "aws_s3_bucket",
            json!({"acl": "private"}),
            json!({"acl": "public-read-write"}),
        ));
        assert_eq!(tf.risk, Risk::High);
    }

    #[test]
    fn instance_class_change_is_medium() {
        let a = assess(&modified(
            "aws_instance",
            json!({"instance_type": "t3.micro"}),
            json!({"instance_type": "t3.large"}),
        ));
        assert_eq!(a.risk, Risk::Medium);
        assert!(has(&a, "t3.micro → t3.large"));
    }

    #[test]
    fn tag_only_change_is_low() {
        let cfn = assess(&modified(
            "AWS::S3::Bucket",
            json!({"Tags": [{"Key": "env", "Value": "dev"}], "BucketName": "b"}),
            json!({"Tags": [{"Key": "env", "Value": "prod"}], "BucketName": "b"}),
        ));
        assert_eq!(cfn.risk, Risk::Low);
        assert_eq!(cfn.reasons, vec!["Tag-only change"]);
        let tf = assess(&modified(
            "aws_instance",
            json!({"tags": {"a": "1"}, "tags_all": {"a": "1"}}),
            json!({"tags": {"a": "2"}, "tags_all": {"a": "2"}}),
        ));
        assert_eq!(tf.risk, Risk::Low);
    }

    #[test]
    fn tag_change_alongside_other_change_is_not_tag_only() {
        let a = assess(&modified(
            "aws_instance",
            json!({"tags": {"a": "1"}, "ami": "x"}),
            json!({"tags": {"a": "2"}, "ami": "y"}),
        ));
        assert_eq!(a.risk, Risk::Medium);
        assert_eq!(a.reasons, vec!["Properties modified"]);
    }

    #[test]
    fn replacement_of_stateful_is_critical() {
        let mut d = modified(
            "aws_db_instance",
            json!({"identifier": "a"}),
            json!({"identifier": "b"}),
        );
        d.forces_replacement = true;
        let a = assess(&d);
        assert_eq!(a.risk, Risk::Critical);
        assert!(has(&a, "replaces the resource"));
        let mut e = modified("aws_instance", json!({"ami": "a"}), json!({"ami": "b"}));
        e.forces_replacement = true;
        assert_eq!(assess(&e).risk, Risk::Medium);
    }

    #[test]
    fn rename_of_stateful_is_high_and_stateless_is_low() {
        let mut d = with_op("aws_s3_bucket", Op::Renamed, json!({}));
        d.renamed_from = Some("aws_s3_bucket.old".into());
        let a = assess(&d);
        assert_eq!(a.risk, Risk::High);
        assert!(has(&a, "Renamed from aws_s3_bucket.old"));
        assert!(has(&a, "moved"));
        let mut e = with_op("aws_iam_role", Op::Renamed, json!({}));
        e.renamed_from = Some("aws_iam_role.old".into());
        assert_eq!(assess(&e).risk, Risk::Low);
    }

    #[test]
    fn highest_finding_wins_and_reasons_are_ordered_by_risk() {
        let a = assess(&modified(
            "aws_db_instance",
            json!({"instance_class": "db.t3.micro", "storage_encrypted": true, "tags": {"a": "1"}}),
            json!({"instance_class": "db.r5.large", "storage_encrypted": false, "tags": {"a": "2"}}),
        ));
        assert_eq!(a.risk, Risk::High);
        assert!(a.reasons[0].starts_with("Encryption disabled"));
        assert!(a.reasons[1].starts_with("Instance class"));
    }

    #[test]
    fn added_stateful_without_encryption_is_medium() {
        let a = assess(&with_op(
            "aws_db_instance",
            Op::Added,
            json!({"storage_encrypted": false}),
        ));
        assert_eq!(a.risk, Risk::Medium);
        assert!(has(&a, "Created with encryption disabled"));
    }
}
