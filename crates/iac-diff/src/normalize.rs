//! Detect what kind of document a parsed tree is and flatten it into the
//! common `Resource` list every later stage works on.

use crate::model::{Detected, DiffError, DocKind, Resource, Side, Syntax};
use crate::parse::{parse_document, Parsed};
use serde_json::{Map, Value};
use std::collections::HashSet;

#[derive(Debug)]
pub struct Normalized {
    pub detected: Detected,
    pub resources: Vec<Resource>,
}

pub fn detect_kind(value: &Value) -> Result<DocKind, DiffError> {
    let Some(obj) = value.as_object() else {
        return Err(DiffError::new(
            "Expected a top-level object (a CloudFormation template or Terraform JSON)",
        ));
    };
    if obj.get("planned_values").is_some() || obj.get("resource_changes").is_some() {
        return Ok(DocKind::TerraformPlan);
    }
    if obj
        .get("values")
        .and_then(|v| v.get("root_module"))
        .is_some()
    {
        return Ok(DocKind::TerraformState);
    }
    if obj.get("resources").map(Value::is_array).unwrap_or(false)
        && obj.get("version").is_some()
        && obj.get("lineage").is_some()
    {
        return Ok(DocKind::TerraformState);
    }
    if obj.get("Resources").map(Value::is_object).unwrap_or(false)
        || obj.get("AWSTemplateFormatVersion").is_some()
    {
        return Ok(DocKind::CloudFormation);
    }
    Err(DiffError::new(
        "Unrecognised document: expected a CloudFormation template (with a Resources map), \
         a `terraform show -json` plan, or a Terraform state file",
    ))
}

pub fn detect(text: &str) -> Result<Detected, DiffError> {
    normalize(text, Side::After).map(|n| n.detected)
}

pub fn normalize(text: &str, side: Side) -> Result<Normalized, DiffError> {
    let Parsed { value, syntax } = parse_document(text)?;
    normalize_value(&value, syntax, side)
}

pub fn normalize_value(value: &Value, syntax: Syntax, side: Side) -> Result<Normalized, DiffError> {
    let kind = detect_kind(value)?;
    let resources = match kind {
        DocKind::CloudFormation => cloudformation_resources(value)?,
        DocKind::TerraformPlan => terraform_plan_resources(value, side),
        DocKind::TerraformState => terraform_state_resources(value),
    };
    let label = match (kind, syntax) {
        (DocKind::CloudFormation, Syntax::Yaml) => "CloudFormation (YAML)".to_string(),
        (DocKind::CloudFormation, Syntax::Json) => "CloudFormation (JSON)".to_string(),
        (DocKind::TerraformPlan, _) => match side {
            Side::Before => "Terraform plan — prior state".to_string(),
            Side::After => "Terraform plan — planned values".to_string(),
        },
        (DocKind::TerraformState, _) => "Terraform state".to_string(),
    };
    Ok(Normalized {
        detected: Detected {
            kind,
            syntax,
            label,
            resource_count: resources.len(),
        },
        resources,
    })
}

// ---------------------------------------------------------------- CloudFormation

/// Resource-level attributes that are not under `Properties` but still matter
/// to a diff. They are folded into the property tree under their own names.
const CFN_FOLDED_ATTRIBUTES: &[&str] = &[
    "DeletionPolicy",
    "UpdateReplacePolicy",
    "DependsOn",
    "Condition",
];

fn cloudformation_resources(value: &Value) -> Result<Vec<Resource>, DiffError> {
    let Some(resources) = value.get("Resources").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(resources.len());
    for (logical_id, body) in resources {
        let Some(body) = body.as_object() else {
            return Err(DiffError::new(format!(
                "Resource `{logical_id}` must be an object with a Type"
            )));
        };
        let kind = body
            .get("Type")
            .and_then(Value::as_str)
            .ok_or_else(|| DiffError::new(format!("Resource `{logical_id}` has no Type")))?
            .to_string();
        let mut properties = match body.get("Properties") {
            Some(Value::Object(props)) => props.clone(),
            Some(_) => {
                return Err(DiffError::new(format!(
                    "Resource `{logical_id}`: Properties must be an object"
                )))
            }
            None => Map::new(),
        };
        for attr in CFN_FOLDED_ATTRIBUTES {
            if let Some(v) = body.get(*attr) {
                properties.insert((*attr).to_string(), v.clone());
            }
        }
        out.push(Resource {
            id: logical_id.clone(),
            kind,
            properties: Value::Object(properties),
            source_kind: DocKind::CloudFormation,
            forces_replacement: false,
        });
    }
    Ok(out)
}

// -------------------------------------------------------------------- Terraform

fn terraform_plan_resources(value: &Value, side: Side) -> Vec<Resource> {
    let module = match side {
        Side::Before => value
            .pointer("/prior_state/values/root_module")
            .or_else(|| value.pointer("/prior_state/root_module")),
        Side::After => value.pointer("/planned_values/root_module"),
    };
    let mut out = Vec::new();
    if let Some(module) = module {
        collect_module(module, DocKind::TerraformPlan, &mut out);
    }
    if side == Side::After {
        let replaced = replaced_addresses(value);
        for r in &mut out {
            r.forces_replacement = replaced.contains(&r.id);
        }
    }
    out
}

/// Addresses whose planned actions are a destroy + create pair.
fn replaced_addresses(plan: &Value) -> HashSet<String> {
    let mut set = HashSet::new();
    let Some(changes) = plan.get("resource_changes").and_then(Value::as_array) else {
        return set;
    };
    for change in changes {
        let actions: Vec<&str> = change
            .pointer("/change/actions")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if actions.contains(&"delete") && actions.contains(&"create") {
            if let Some(addr) = change.get("address").and_then(Value::as_str) {
                set.insert(addr.to_string());
            }
        }
    }
    set
}

fn terraform_state_resources(value: &Value) -> Vec<Resource> {
    let mut out = Vec::new();
    if let Some(module) = value.pointer("/values/root_module") {
        collect_module(module, DocKind::TerraformState, &mut out);
    } else if let Some(resources) = value.get("resources").and_then(Value::as_array) {
        collect_raw_state(resources, &mut out);
    }
    out
}

/// `terraform show -json` module shape, recursing into `child_modules`.
fn collect_module(module: &Value, source: DocKind, out: &mut Vec<Resource>) {
    if let Some(resources) = module.get("resources").and_then(Value::as_array) {
        for r in resources {
            if r.get("mode").and_then(Value::as_str) == Some("data") {
                continue;
            }
            let (Some(address), Some(kind)) = (
                r.get("address").and_then(Value::as_str),
                r.get("type").and_then(Value::as_str),
            ) else {
                continue;
            };
            let values = r
                .get("values")
                .cloned()
                .unwrap_or(Value::Object(Map::new()));
            out.push(Resource {
                id: address.to_string(),
                kind: kind.to_string(),
                properties: expand_policy_strings(values),
                source_kind: source,
                forces_replacement: false,
            });
        }
    }
    if let Some(children) = module.get("child_modules").and_then(Value::as_array) {
        for child in children {
            collect_module(child, source, out);
        }
    }
}

/// Raw `terraform.tfstate` (format version 4): `resources[].instances[].attributes`.
fn collect_raw_state(resources: &[Value], out: &mut Vec<Resource>) {
    for r in resources {
        if r.get("mode").and_then(Value::as_str) == Some("data") {
            continue;
        }
        let (Some(kind), Some(name)) = (
            r.get("type").and_then(Value::as_str),
            r.get("name").and_then(Value::as_str),
        ) else {
            continue;
        };
        let module_prefix = r
            .get("module")
            .and_then(Value::as_str)
            .map(|m| format!("{m}."))
            .unwrap_or_default();
        let Some(instances) = r.get("instances").and_then(Value::as_array) else {
            continue;
        };
        for inst in instances {
            let index = match inst.get("index_key") {
                Some(Value::String(s)) => format!("[\"{s}\"]"),
                Some(Value::Number(n)) => format!("[{n}]"),
                _ => String::new(),
            };
            let attrs = inst
                .get("attributes")
                .cloned()
                .unwrap_or(Value::Object(Map::new()));
            out.push(Resource {
                id: format!("{module_prefix}{kind}.{name}{index}"),
                kind: kind.to_string(),
                properties: expand_policy_strings(attrs),
                source_kind: DocKind::TerraformState,
                forces_replacement: false,
            });
        }
    }
}

/// Terraform carries IAM policy documents as JSON strings. Parsing them lets the
/// property diff and the IAM risk rule see the statements, not an opaque blob.
fn expand_policy_strings(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    let v = if is_policy_key(&k) {
                        parse_embedded_json(v)
                    } else {
                        v
                    };
                    (k, expand_policy_strings(v))
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(expand_policy_strings).collect()),
        other => other,
    }
}

fn is_policy_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k == "policy" || k.ends_with("_policy") || k.ends_with("policy_document")
}

fn parse_embedded_json(value: Value) -> Value {
    match &value {
        Value::String(s) if s.trim_start().starts_with('{') => {
            serde_json::from_str::<Value>(s).unwrap_or(value)
        }
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ids(resources: &[Resource]) -> Vec<&str> {
        resources.iter().map(|r| r.id.as_str()).collect()
    }

    #[test]
    fn detects_cloudformation_yaml() {
        let d = detect(
            "AWSTemplateFormatVersion: '2010-09-09'\nResources:\n  B:\n    Type: AWS::S3::Bucket\n",
        )
        .unwrap();
        assert_eq!(d.kind, DocKind::CloudFormation);
        assert_eq!(d.syntax, Syntax::Yaml);
        assert_eq!(d.resource_count, 1);
        assert_eq!(d.label, "CloudFormation (YAML)");
    }

    #[test]
    fn detects_cloudformation_json() {
        let d = detect(r#"{"Resources": {"B": {"Type": "AWS::S3::Bucket"}}}"#).unwrap();
        assert_eq!(d.kind, DocKind::CloudFormation);
        assert_eq!(d.syntax, Syntax::Json);
    }

    #[test]
    fn detects_terraform_plan_and_state() {
        let plan = r#"{"format_version":"1.2","planned_values":{"root_module":{"resources":[]}}}"#;
        assert_eq!(detect(plan).unwrap().kind, DocKind::TerraformPlan);
        let state = r#"{"format_version":"1.0","values":{"root_module":{"resources":[]}}}"#;
        assert_eq!(detect(state).unwrap().kind, DocKind::TerraformState);
        let raw = r#"{"version":4,"terraform_version":"1.9.0","lineage":"abc","resources":[]}"#;
        assert_eq!(detect(raw).unwrap().kind, DocKind::TerraformState);
    }

    #[test]
    fn rejects_unknown_shapes() {
        assert!(detect(r#"{"hello": "world"}"#).is_err());
        assert!(detect("[1, 2, 3]").is_err());
        assert!(detect("just a string").is_err());
    }

    #[test]
    fn cloudformation_folds_resource_attributes_into_properties() {
        let doc = "Resources:\n  Db:\n    Type: AWS::RDS::DBInstance\n    DeletionPolicy: Retain\n    DependsOn: [Vpc]\n    Properties:\n      DBInstanceClass: db.t3.micro\n  NoProps:\n    Type: AWS::SNS::Topic\n";
        let n = normalize(doc, Side::After).unwrap();
        assert_eq!(ids(&n.resources), vec!["Db", "NoProps"]);
        let db = &n.resources[0];
        assert_eq!(db.kind, "AWS::RDS::DBInstance");
        assert_eq!(
            db.properties,
            json!({"DBInstanceClass": "db.t3.micro", "DeletionPolicy": "Retain", "DependsOn": ["Vpc"]})
        );
        assert_eq!(n.resources[1].properties, json!({}));
    }

    #[test]
    fn cloudformation_resource_without_type_is_an_error() {
        let err =
            normalize("Resources:\n  Broken:\n    Properties: {}\n", Side::After).unwrap_err();
        assert!(err.error.contains("Broken"));
    }

    #[test]
    fn terraform_plan_reads_prior_state_before_and_planned_values_after() {
        let plan = json!({
            "format_version": "1.2",
            "prior_state": {"values": {"root_module": {"resources": [
                {"address": "aws_instance.web", "type": "aws_instance", "name": "web", "mode": "managed",
                 "values": {"instance_type": "t3.micro"}}
            ]}}},
            "planned_values": {"root_module": {"resources": [
                {"address": "aws_instance.web", "type": "aws_instance", "name": "web", "mode": "managed",
                 "values": {"instance_type": "t3.large"}},
                {"address": "data.aws_ami.ubuntu", "type": "aws_ami", "name": "ubuntu", "mode": "data", "values": {}}
            ]}},
            "resource_changes": [
                {"address": "aws_instance.web", "change": {"actions": ["delete", "create"]}}
            ]
        })
        .to_string();
        let before = normalize(&plan, Side::Before).unwrap();
        let after = normalize(&plan, Side::After).unwrap();
        assert_eq!(before.resources[0].properties["instance_type"], "t3.micro");
        assert!(!before.resources[0].forces_replacement);
        assert_eq!(after.resources.len(), 1, "data sources are skipped");
        assert_eq!(after.resources[0].properties["instance_type"], "t3.large");
        assert!(after.resources[0].forces_replacement);
        assert_eq!(before.detected.label, "Terraform plan — prior state");
        assert_eq!(after.detected.label, "Terraform plan — planned values");
    }

    #[test]
    fn terraform_state_recurses_into_child_modules() {
        let state = json!({
            "format_version": "1.0",
            "values": {"root_module": {
                "resources": [{"address": "aws_vpc.main", "type": "aws_vpc", "name": "main", "values": {"cidr_block": "10.0.0.0/16"}}],
                "child_modules": [{
                    "address": "module.db",
                    "resources": [{"address": "module.db.aws_db_instance.main", "type": "aws_db_instance", "name": "main", "values": {}}],
                    "child_modules": [{
                        "address": "module.db.module.backup",
                        "resources": [{"address": "module.db.module.backup.aws_backup_vault.v", "type": "aws_backup_vault", "name": "v", "values": {}}]
                    }]
                }]
            }}
        })
        .to_string();
        let n = normalize(&state, Side::After).unwrap();
        assert_eq!(
            ids(&n.resources),
            vec![
                "aws_vpc.main",
                "module.db.aws_db_instance.main",
                "module.db.module.backup.aws_backup_vault.v"
            ]
        );
    }

    #[test]
    fn raw_tfstate_builds_addresses_with_module_and_index() {
        let raw = json!({
            "version": 4, "terraform_version": "1.9.0", "lineage": "x",
            "resources": [
                {"mode": "managed", "type": "aws_s3_bucket", "name": "logs", "instances": [{"attributes": {"bucket": "logs"}}]},
                {"module": "module.net", "mode": "managed", "type": "aws_subnet", "name": "private",
                 "instances": [{"index_key": 0, "attributes": {}}, {"index_key": "b", "attributes": {}}]},
                {"mode": "data", "type": "aws_ami", "name": "x", "instances": [{"attributes": {}}]}
            ]
        })
        .to_string();
        let n = normalize(&raw, Side::After).unwrap();
        assert_eq!(
            ids(&n.resources),
            vec![
                "aws_s3_bucket.logs",
                "module.net.aws_subnet.private[0]",
                "module.net.aws_subnet.private[\"b\"]"
            ]
        );
        assert_eq!(n.resources[0].properties, json!({"bucket": "logs"}));
    }

    #[test]
    fn embedded_policy_strings_are_parsed() {
        let state = json!({
            "format_version": "1.0",
            "values": {"root_module": {"resources": [{
                "address": "aws_iam_role.app", "type": "aws_iam_role", "name": "app",
                "values": {
                    "assume_role_policy": "{\"Version\":\"2012-10-17\",\"Statement\":[]}",
                    "inline_policy": [{"name": "p", "policy": "{\"Statement\":[{\"Action\":\"s3:GetObject\"}]}"}],
                    "description": "{not a policy}"
                }
            }]}}
        })
        .to_string();
        let n = normalize(&state, Side::After).unwrap();
        let props = &n.resources[0].properties;
        assert_eq!(props["assume_role_policy"]["Version"], "2012-10-17");
        assert_eq!(
            props["inline_policy"][0]["policy"]["Statement"][0]["Action"],
            "s3:GetObject"
        );
        assert_eq!(props["description"], "{not a policy}");
    }

    #[test]
    fn malformed_policy_string_is_left_as_is() {
        let v = expand_policy_strings(json!({"policy": "{oops"}));
        assert_eq!(v, json!({"policy": "{oops"}));
    }
}
