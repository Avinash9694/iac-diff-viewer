//! Data model shared by every stage: detection, normalisation, diff, risk.
//! Everything here serialises to the JSON the TypeScript shell consumes.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DocKind {
    CloudFormation,
    TerraformPlan,
    TerraformState,
}

impl DocKind {
    pub fn label(self) -> &'static str {
        match self {
            DocKind::CloudFormation => "CloudFormation template",
            DocKind::TerraformPlan => "Terraform plan",
            DocKind::TerraformState => "Terraform state",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Syntax {
    Json,
    Yaml,
}

/// Which half of a diff a document is being read for. Only matters for a
/// Terraform plan, which carries both its prior state and its planned values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Before,
    After,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detected {
    pub kind: DocKind,
    pub syntax: Syntax,
    pub label: String,
    pub resource_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    /// Logical id (CloudFormation) or address (Terraform).
    pub id: String,
    /// `AWS::S3::Bucket`, `aws_s3_bucket`, ...
    pub kind: String,
    pub properties: Value,
    pub source_kind: DocKind,
    /// Terraform plan only: this address is destroyed and re-created.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forces_replacement: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    Added,
    Removed,
    Modified,
    Renamed,
    Unchanged,
}

impl Op {
    /// Display order within a risk band: destructive first.
    pub fn rank(self) -> u8 {
        match self {
            Op::Removed => 0,
            Op::Modified => 1,
            Op::Renamed => 2,
            Op::Added => 3,
            Op::Unchanged => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeOp {
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyChange {
    /// Dotted path with `[i]` array segments, e.g. `Tags[1].Value`.
    /// For a modified or added leaf this is the path on the `after` side.
    pub path: String,
    /// Set only when the matching element sits at a different index before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_path: Option<String>,
    pub op: ChangeOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
}

impl PropertyChange {
    pub fn leaf(&self) -> &str {
        last_segment(&self.path)
    }

    pub fn path_on(&self, side: Side) -> &str {
        match side {
            Side::Before => self.before_path.as_deref().unwrap_or(&self.path),
            Side::After => &self.path,
        }
    }
}

/// Final key segment of a dotted path, ignoring any trailing `[i]`.
pub fn last_segment(path: &str) -> &str {
    let tail = path.rsplit('.').next().unwrap_or(path);
    match tail.find('[') {
        Some(i) => &tail[..i],
        None => tail,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceDiff {
    /// Id on the `after` side (the `before` id for a removed resource).
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
    pub kind: String,
    pub op: Op,
    pub risk: Risk,
    pub reasons: Vec<String>,
    pub changes: Vec<PropertyChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forces_replacement: bool,
}

impl ResourceDiff {
    pub fn sort_key(&self) -> (std::cmp::Reverse<Risk>, u8, &str) {
        (std::cmp::Reverse(self.risk), self.op.rank(), &self.id)
    }
}

pub fn order_resources(resources: &mut [ResourceDiff]) {
    resources.sort_by(|a, b| {
        let ord = a.sort_key().cmp(&b.sort_key());
        if ord == Ordering::Equal {
            a.id.cmp(&b.id)
        } else {
            ord
        }
    });
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpCounts {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub renamed: usize,
    pub unchanged: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskCounts {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub critical: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub total: usize,
    pub changed: usize,
    pub by_op: OpCounts,
    /// Risk counts cover changed resources only; an unchanged row carries no risk.
    pub by_risk: RiskCounts,
    pub overall_risk: Risk,
}

impl Summary {
    pub fn from_resources(resources: &[ResourceDiff]) -> Summary {
        let mut by_op = OpCounts::default();
        let mut by_risk = RiskCounts::default();
        let mut overall = Risk::Low;
        for r in resources {
            match r.op {
                Op::Added => by_op.added += 1,
                Op::Removed => by_op.removed += 1,
                Op::Modified => by_op.modified += 1,
                Op::Renamed => by_op.renamed += 1,
                Op::Unchanged => by_op.unchanged += 1,
            }
            if r.op == Op::Unchanged {
                continue;
            }
            match r.risk {
                Risk::Low => by_risk.low += 1,
                Risk::Medium => by_risk.medium += 1,
                Risk::High => by_risk.high += 1,
                Risk::Critical => by_risk.critical += 1,
            }
            overall = overall.max(r.risk);
        }
        Summary {
            total: resources.len(),
            changed: resources.len() - by_op.unchanged,
            by_op,
            by_risk,
            overall_risk: overall,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffReport {
    pub before: Detected,
    pub after: Detected,
    pub summary: Summary,
    pub resources: Vec<ResourceDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffError {
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    /// `before` / `after` / `document` — which input the error belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
}

impl DiffError {
    pub fn new(message: impl Into<String>) -> Self {
        DiffError {
            error: message.into(),
            line: None,
            column: None,
            side: None,
        }
    }

    pub fn at(mut self, line: usize, column: usize) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    pub fn on_side(mut self, side: &str) -> Self {
        self.side = Some(side.to_string());
        self
    }
}

impl std::fmt::Display for DiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(f, "{} (line {}, column {})", self.error, l, c),
            _ => write!(f, "{}", self.error),
        }
    }
}

impl std::error::Error for DiffError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_segment_strips_array_index() {
        assert_eq!(last_segment("Tags[1].Value"), "Value");
        assert_eq!(
            last_segment("SecurityGroupIngress[0]"),
            "SecurityGroupIngress"
        );
        assert_eq!(last_segment("InstanceType"), "InstanceType");
    }

    #[test]
    fn risk_orders_low_to_critical() {
        assert!(Risk::Low < Risk::Medium);
        assert!(Risk::Medium < Risk::High);
        assert!(Risk::High < Risk::Critical);
    }

    #[test]
    fn summary_ignores_unchanged_for_risk() {
        let mk = |op, risk| ResourceDiff {
            id: "x".into(),
            renamed_from: None,
            kind: "k".into(),
            op,
            risk,
            reasons: vec![],
            changes: vec![],
            before: None,
            after: None,
            forces_replacement: false,
        };
        let s = Summary::from_resources(&[
            mk(Op::Unchanged, Risk::Low),
            mk(Op::Removed, Risk::Critical),
            mk(Op::Added, Risk::Low),
        ]);
        assert_eq!(s.total, 3);
        assert_eq!(s.changed, 2);
        assert_eq!(s.by_op.unchanged, 1);
        assert_eq!(s.by_risk.low, 1);
        assert_eq!(s.by_risk.critical, 1);
        assert_eq!(s.overall_risk, Risk::Critical);
    }
}
