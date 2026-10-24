//! Ties the stages together: normalise both documents, match resources by
//! id, diff properties, fold rename pairs, tag risk, order and summarise.

use crate::model::{
    order_resources, DiffError, DiffReport, DocKind, Op, Resource, ResourceDiff, Risk, Side,
    Summary,
};
use crate::normalize::{normalize, Normalized};
use crate::propdiff::diff_values;
use crate::rename::detect_renames;
use crate::risk::assess;
use std::collections::HashMap;

pub fn diff_documents(before: &str, after: &str) -> Result<DiffReport, DiffError> {
    let (before_text, after_text) = resolve_single_plan(before, after);
    let before = normalize(before_text, Side::Before).map_err(|e| e.on_side("before"))?;
    let after = normalize(after_text, Side::After).map_err(|e| e.on_side("after"))?;
    Ok(diff_normalized(before, after))
}

/// A Terraform plan carries both halves of the diff. If only one side was
/// supplied and it is a plan, read its prior state as `before` and its
/// planned values as `after`.
fn resolve_single_plan<'a>(before: &'a str, after: &'a str) -> (&'a str, &'a str) {
    let before_empty = before.trim().is_empty();
    let after_empty = after.trim().is_empty();
    if after_empty && !before_empty && is_plan(before) {
        return (before, before);
    }
    if before_empty && !after_empty && is_plan(after) {
        return (after, after);
    }
    (before, after)
}

fn is_plan(text: &str) -> bool {
    crate::normalize::detect(text)
        .map(|d| d.kind == DocKind::TerraformPlan)
        .unwrap_or(false)
}

pub fn diff_normalized(before: Normalized, after: Normalized) -> DiffReport {
    let mut resources = diff_resources(&before.resources, &after.resources);
    for r in &mut resources {
        let assessment = assess(r);
        r.risk = assessment.risk;
        r.reasons = assessment.reasons;
    }
    order_resources(&mut resources);
    DiffReport {
        before: before.detected,
        after: after.detected,
        summary: Summary::from_resources(&resources),
        resources,
    }
}

fn diff_resources(before: &[Resource], after: &[Resource]) -> Vec<ResourceDiff> {
    let before_by_id: HashMap<&str, &Resource> =
        before.iter().map(|r| (r.id.as_str(), r)).collect();
    let after_by_id: HashMap<&str, &Resource> = after.iter().map(|r| (r.id.as_str(), r)).collect();

    let mut out = Vec::new();
    let mut added: Vec<&Resource> = Vec::new();
    let mut removed: Vec<&Resource> = Vec::new();

    for a in after {
        match before_by_id.get(a.id.as_str()) {
            Some(b) if b.kind == a.kind => {
                let changes = diff_values(&b.properties, &a.properties);
                let op = if changes.is_empty() && !a.forces_replacement {
                    Op::Unchanged
                } else {
                    Op::Modified
                };
                out.push(ResourceDiff {
                    id: a.id.clone(),
                    renamed_from: None,
                    kind: a.kind.clone(),
                    op,
                    risk: Risk::Low,
                    reasons: Vec::new(),
                    changes,
                    before: Some(b.properties.clone()),
                    after: Some(a.properties.clone()),
                    forces_replacement: a.forces_replacement,
                });
            }
            // Same id, different type: that is a removal plus an addition.
            Some(b) => {
                removed.push(b);
                added.push(a);
            }
            None => added.push(a),
        }
    }
    for b in before {
        if !after_by_id.contains_key(b.id.as_str()) {
            removed.push(b);
        }
    }

    let pairs = detect_renames(&removed, &added);
    let mut claimed_removed = vec![false; removed.len()];
    let mut claimed_added = vec![false; added.len()];
    for (ri, ai) in pairs {
        claimed_removed[ri] = true;
        claimed_added[ai] = true;
        let (from, to) = (removed[ri], added[ai]);
        out.push(ResourceDiff {
            id: to.id.clone(),
            renamed_from: Some(from.id.clone()),
            kind: to.kind.clone(),
            op: Op::Renamed,
            risk: Risk::Low,
            reasons: Vec::new(),
            changes: diff_values(&from.properties, &to.properties),
            before: Some(from.properties.clone()),
            after: Some(to.properties.clone()),
            forces_replacement: to.forces_replacement,
        });
    }
    for (i, r) in removed.iter().enumerate() {
        if !claimed_removed[i] {
            out.push(ResourceDiff {
                id: r.id.clone(),
                renamed_from: None,
                kind: r.kind.clone(),
                op: Op::Removed,
                risk: Risk::Low,
                reasons: Vec::new(),
                changes: Vec::new(),
                before: Some(r.properties.clone()),
                after: None,
                forces_replacement: false,
            });
        }
    }
    for (i, a) in added.iter().enumerate() {
        if !claimed_added[i] {
            out.push(ResourceDiff {
                id: a.id.clone(),
                renamed_from: None,
                kind: a.kind.clone(),
                op: Op::Added,
                risk: Risk::Low,
                reasons: Vec::new(),
                changes: Vec::new(),
                before: None,
                after: Some(a.properties.clone()),
                forces_replacement: false,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CFN_BEFORE: &str = r#"
AWSTemplateFormatVersion: '2010-09-09'
Resources:
  Vpc:
    Type: AWS::EC2::VPC
    Properties:
      CidrBlock: 10.0.0.0/16
  WebSg:
    Type: AWS::EC2::SecurityGroup
    Properties:
      VpcId: !Ref Vpc
      SecurityGroupIngress:
        - IpProtocol: tcp
          FromPort: 22
          ToPort: 22
          CidrIp: 10.0.0.0/8
  Db:
    Type: AWS::RDS::DBInstance
    DeletionPolicy: Snapshot
    Properties:
      DBInstanceClass: db.t3.micro
      StorageEncrypted: true
  Assets:
    Type: AWS::S3::Bucket
    Properties:
      BucketName: !Sub '${AWS::StackName}-assets'
      Tags:
        - Key: team
          Value: web
"#;

    const CFN_AFTER: &str = r#"
AWSTemplateFormatVersion: '2010-09-09'
Resources:
  Vpc:
    Type: AWS::EC2::VPC
    Properties:
      CidrBlock: 10.0.0.0/16
  WebSg:
    Type: AWS::EC2::SecurityGroup
    Properties:
      VpcId: !Ref Vpc
      SecurityGroupIngress:
        - IpProtocol: tcp
          FromPort: 22
          ToPort: 22
          CidrIp: 0.0.0.0/0
  StaticAssets:
    Type: AWS::S3::Bucket
    Properties:
      BucketName: !Sub '${AWS::StackName}-assets'
      Tags:
        - Key: team
          Value: web
  Fn:
    Type: AWS::Lambda::Function
    Properties:
      Runtime: nodejs20.x
"#;

    fn find<'a>(report: &'a DiffReport, id: &str) -> &'a ResourceDiff {
        report
            .resources
            .iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("no resource {id}"))
    }

    #[test]
    fn cloudformation_end_to_end() {
        let report = diff_documents(CFN_BEFORE, CFN_AFTER).unwrap();
        assert_eq!(report.before.kind, DocKind::CloudFormation);
        assert_eq!(report.summary.total, 5);
        assert_eq!(report.summary.by_op.unchanged, 1);
        assert_eq!(report.summary.by_op.removed, 1);
        assert_eq!(report.summary.by_op.modified, 1);
        assert_eq!(report.summary.by_op.renamed, 1);
        assert_eq!(report.summary.by_op.added, 1);
        assert_eq!(report.summary.overall_risk, Risk::Critical);

        assert_eq!(find(&report, "Vpc").op, Op::Unchanged);
        let db = find(&report, "Db");
        assert_eq!(db.op, Op::Removed);
        assert_eq!(db.risk, Risk::Critical);
        let sg = find(&report, "WebSg");
        assert_eq!(sg.op, Op::Modified);
        assert_eq!(sg.risk, Risk::Critical);
        assert_eq!(sg.changes.len(), 1);
        assert_eq!(sg.changes[0].path, "SecurityGroupIngress[0].CidrIp");
        let assets = find(&report, "StaticAssets");
        assert_eq!(assets.op, Op::Renamed);
        assert_eq!(assets.renamed_from.as_deref(), Some("Assets"));
        assert_eq!(assets.risk, Risk::High);
        assert_eq!(find(&report, "Fn").risk, Risk::Low);
    }

    #[test]
    fn resources_are_ordered_by_risk_then_op() {
        let report = diff_documents(CFN_BEFORE, CFN_AFTER).unwrap();
        let ids: Vec<&str> = report.resources.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["Db", "WebSg", "StaticAssets", "Fn", "Vpc"]);
    }

    #[test]
    fn single_plan_document_diffs_prior_state_against_planned_values() {
        let plan = json!({
            "format_version": "1.2",
            "prior_state": {"values": {"root_module": {"resources": [
                {"address": "aws_instance.web", "type": "aws_instance", "name": "web", "values": {"instance_type": "t3.micro", "tags": {"env": "dev"}}}
            ]}}},
            "planned_values": {"root_module": {"resources": [
                {"address": "aws_instance.web", "type": "aws_instance", "name": "web", "values": {"instance_type": "t3.large", "tags": {"env": "dev"}}},
                {"address": "aws_s3_bucket.new", "type": "aws_s3_bucket", "name": "new", "values": {"bucket": "new"}}
            ]}},
            "resource_changes": []
        })
        .to_string();
        let report = diff_documents(&plan, "").unwrap();
        assert_eq!(report.before.label, "Terraform plan — prior state");
        assert_eq!(report.after.label, "Terraform plan — planned values");
        assert_eq!(find(&report, "aws_instance.web").risk, Risk::Medium);
        assert_eq!(find(&report, "aws_s3_bucket.new").op, Op::Added);
        let same = diff_documents(&plan, &plan).unwrap();
        assert_eq!(same.summary, report.summary);
        let reversed = diff_documents("", &plan).unwrap();
        assert_eq!(reversed.summary, report.summary);
    }

    #[test]
    fn same_id_with_different_type_is_remove_plus_add() {
        let before = r#"{"Resources": {"Thing": {"Type": "AWS::SQS::Queue"}}}"#;
        let after = r#"{"Resources": {"Thing": {"Type": "AWS::SNS::Topic"}}}"#;
        let report = diff_documents(before, after).unwrap();
        let ops: Vec<(String, Op)> = report
            .resources
            .iter()
            .map(|r| (r.kind.clone(), r.op))
            .collect();
        assert!(ops.contains(&("AWS::SQS::Queue".to_string(), Op::Removed)));
        assert!(ops.contains(&("AWS::SNS::Topic".to_string(), Op::Added)));
        assert_eq!(report.summary.overall_risk, Risk::Critical);
    }

    #[test]
    fn errors_name_the_side() {
        let err = diff_documents("Resources: {}", "{ nope").unwrap_err();
        assert_eq!(err.side.as_deref(), Some("after"));
        let err = diff_documents("", "Resources: {}").unwrap_err();
        assert_eq!(err.side.as_deref(), Some("before"));
        assert_eq!(err.error, "Document is empty");
    }

    #[test]
    fn identical_documents_are_all_unchanged_and_low() {
        let report = diff_documents(CFN_BEFORE, CFN_BEFORE).unwrap();
        assert_eq!(report.summary.changed, 0);
        assert_eq!(report.summary.overall_risk, Risk::Low);
        assert!(report.resources.iter().all(|r| r.op == Op::Unchanged));
    }

    #[test]
    fn replacement_without_property_change_still_counts_as_modified() {
        let plan = json!({
            "format_version": "1.2",
            "prior_state": {"values": {"root_module": {"resources": [
                {"address": "aws_db_instance.main", "type": "aws_db_instance", "name": "main", "values": {"identifier": "db"}}
            ]}}},
            "planned_values": {"root_module": {"resources": [
                {"address": "aws_db_instance.main", "type": "aws_db_instance", "name": "main", "values": {"identifier": "db"}}
            ]}},
            "resource_changes": [{"address": "aws_db_instance.main", "change": {"actions": ["delete", "create"]}}]
        })
        .to_string();
        let report = diff_documents(&plan, "").unwrap();
        let db = find(&report, "aws_db_instance.main");
        assert_eq!(db.op, Op::Modified);
        assert!(db.forces_replacement);
        assert_eq!(db.risk, Risk::Critical);
    }
}
