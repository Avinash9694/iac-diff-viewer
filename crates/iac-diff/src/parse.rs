//! Text → `serde_json::Value`. YAML goes through a tag pass that rewrites the
//! CloudFormation short-form intrinsics (`!Ref`, `!GetAtt`, `!Sub`, ...) into
//! their long-form objects so both syntaxes normalise to the same tree.

use crate::model::{DiffError, Syntax};
use serde_json::{Map, Value};
use serde_yaml_ng as yaml;

#[derive(Debug)]
pub struct Parsed {
    pub value: Value,
    pub syntax: Syntax,
}

pub fn parse_document(text: &str) -> Result<Parsed, DiffError> {
    let trimmed = text.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err(DiffError::new("Document is empty"));
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return parse_json(trimmed);
    }
    match parse_yaml(trimmed) {
        Ok(parsed) => Ok(parsed),
        // A document that is almost JSON (stray trailing comma, comment) gives a
        // better message from the JSON parser than from YAML.
        Err(yaml_err) => match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => Ok(Parsed {
                value,
                syntax: Syntax::Json,
            }),
            Err(_) => Err(yaml_err),
        },
    }
}

fn parse_json(text: &str) -> Result<Parsed, DiffError> {
    serde_json::from_str::<Value>(text)
        .map(|value| Parsed {
            value,
            syntax: Syntax::Json,
        })
        .map_err(|e| {
            DiffError::new(format!("Invalid JSON: {}", strip_position(&e.to_string())))
                .at(e.line(), e.column())
        })
}

fn parse_yaml(text: &str) -> Result<Parsed, DiffError> {
    let doc: yaml::Value = yaml::from_str(text).map_err(|e| {
        let base = DiffError::new(format!("Invalid YAML: {}", strip_position(&e.to_string())));
        match e.location() {
            Some(loc) => base.at(loc.line(), loc.column()),
            None => base,
        }
    })?;
    let value = yaml_to_json(doc)?;
    Ok(Parsed {
        value,
        syntax: Syntax::Yaml,
    })
}

/// Parser messages end in ` at line N column M`; the structured fields carry that.
fn strip_position(message: &str) -> &str {
    match message.rfind(" at line ") {
        Some(i) => &message[..i],
        None => message,
    }
}

pub fn yaml_to_json(value: yaml::Value) -> Result<Value, DiffError> {
    Ok(match value {
        yaml::Value::Null => Value::Null,
        yaml::Value::Bool(b) => Value::Bool(b),
        yaml::Value::Number(n) => yaml_number(n),
        yaml::Value::String(s) => Value::String(s),
        yaml::Value::Sequence(items) => Value::Array(
            items
                .into_iter()
                .map(yaml_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        yaml::Value::Mapping(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                out.insert(key_to_string(k), yaml_to_json(v)?);
            }
            Value::Object(out)
        }
        yaml::Value::Tagged(tagged) => {
            let tag = tagged.tag.to_string();
            let inner = yaml_to_json(tagged.value)?;
            intrinsic_to_long_form(&tag, inner)
        }
    })
}

fn yaml_number(n: yaml::Number) -> Value {
    if let Some(i) = n.as_i64() {
        Value::from(i)
    } else if let Some(u) = n.as_u64() {
        Value::from(u)
    } else if let Some(f) = n.as_f64() {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(n.to_string()))
    } else {
        Value::String(n.to_string())
    }
}

fn key_to_string(key: yaml::Value) -> String {
    match key {
        yaml::Value::String(s) => s,
        yaml::Value::Number(n) => n.to_string(),
        yaml::Value::Bool(b) => b.to_string(),
        yaml::Value::Null => "null".to_string(),
        other => {
            serde_json::to_string(&yaml_to_json(other).unwrap_or(Value::Null)).unwrap_or_default()
        }
    }
}

/// `!Ref X` → `{"Ref": "X"}`, `!GetAtt A.B` → `{"Fn::GetAtt": ["A", "B"]}`,
/// `!Condition C` → `{"Condition": "C"}`, anything else `!Name v` → `{"Fn::Name": v}`.
pub fn intrinsic_to_long_form(tag: &str, value: Value) -> Value {
    let name = tag.trim_start_matches('!');
    let mut obj = Map::new();
    match name {
        "Ref" => {
            obj.insert("Ref".into(), value);
        }
        "Condition" => {
            obj.insert("Condition".into(), value);
        }
        "GetAtt" => {
            let parts = match value {
                Value::String(s) => match s.split_once('.') {
                    Some((logical, attr)) => Value::Array(vec![
                        Value::String(logical.to_string()),
                        Value::String(attr.to_string()),
                    ]),
                    None => Value::Array(vec![Value::String(s)]),
                },
                other => other,
            };
            obj.insert("Fn::GetAtt".into(), parts);
        }
        _ => {
            obj.insert(format!("Fn::{name}"), value);
        }
    }
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_json_object() {
        let p = parse_document(r#"{"a": 1, "b": [true, null]}"#).unwrap();
        assert_eq!(p.syntax, Syntax::Json);
        assert_eq!(p.value, json!({"a": 1, "b": [true, null]}));
    }

    #[test]
    fn json_error_carries_position() {
        let err = parse_document("{\n  \"a\": 1,\n  \"b\": \n}").unwrap_err();
        assert!(err.error.starts_with("Invalid JSON"), "{}", err.error);
        assert_eq!(err.line, Some(4));
        assert!(err.column.is_some());
        assert!(!err.error.contains("at line"));
    }

    #[test]
    fn yaml_error_carries_position() {
        let err = parse_document("Resources:\n  A:\n    Type: x\n   bad: [unclosed").unwrap_err();
        assert!(err.error.starts_with("Invalid YAML"), "{}", err.error);
        assert!(err.line.is_some());
    }

    #[test]
    fn empty_document_is_an_error() {
        let err = parse_document("   \n").unwrap_err();
        assert_eq!(err.error, "Document is empty");
    }

    #[test]
    fn yaml_scalars_and_keys_convert() {
        let p = parse_document("a: 1\nb: 2.5\nc: true\n3: three\nd: ~\n").unwrap();
        assert_eq!(p.syntax, Syntax::Yaml);
        assert_eq!(
            p.value,
            json!({"a": 1, "b": 2.5, "c": true, "3": "three", "d": null})
        );
    }

    #[test]
    fn ref_tag_becomes_long_form() {
        let p = parse_document("VpcId: !Ref Vpc\n").unwrap();
        assert_eq!(p.value, json!({"VpcId": {"Ref": "Vpc"}}));
    }

    #[test]
    fn getatt_string_form_splits_on_first_dot() {
        let p =
            parse_document("Arn: !GetAtt Role.Arn\nNested: !GetAtt Fn.Outputs.Value\n").unwrap();
        assert_eq!(
            p.value,
            json!({
                "Arn": {"Fn::GetAtt": ["Role", "Arn"]},
                "Nested": {"Fn::GetAtt": ["Fn", "Outputs.Value"]}
            })
        );
    }

    #[test]
    fn getatt_sequence_form_passes_through() {
        let p = parse_document("Arn: !GetAtt [Role, Arn]\n").unwrap();
        assert_eq!(p.value, json!({"Arn": {"Fn::GetAtt": ["Role", "Arn"]}}));
    }

    #[test]
    fn sub_join_and_condition_tags() {
        let doc = "Name: !Sub '${AWS::StackName}-bucket'\nList: !Join [',', [a, !Ref B]]\nCond: !Condition IsProd\nSel: !Select [0, !GetAZs '']\n";
        let p = parse_document(doc).unwrap();
        assert_eq!(
            p.value,
            json!({
                "Name": {"Fn::Sub": "${AWS::StackName}-bucket"},
                "List": {"Fn::Join": [",", ["a", {"Ref": "B"}]]},
                "Cond": {"Condition": "IsProd"},
                "Sel": {"Fn::Select": [0, {"Fn::GetAZs": ""}]}
            })
        );
    }

    #[test]
    fn long_form_yaml_matches_short_form() {
        let short = parse_document("A: !Ref X\nB: !GetAtt Y.Z\n").unwrap();
        let long = parse_document("A:\n  Ref: X\nB:\n  Fn::GetAtt: [Y, Z]\n").unwrap();
        assert_eq!(short.value, long.value);
    }

    #[test]
    fn near_json_with_leading_whitespace_parses_as_json() {
        let p = parse_document("\n\n  {\"Resources\": {}}").unwrap();
        assert_eq!(p.syntax, Syntax::Json);
    }
}
