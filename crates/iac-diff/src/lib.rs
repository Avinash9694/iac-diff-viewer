//! `iac-diff`: semantic, resource-level diff for CloudFormation and Terraform
//! documents with rule-based risk tagging. Compiled to WebAssembly for the
//! browser; the Rust API (`diff_documents`, `detect`) is usable natively too.

pub mod engine;
pub mod model;
pub mod normalize;
pub mod parse;
pub mod propdiff;
pub mod rename;
pub mod risk;

pub use engine::diff_documents;
pub use model::{Detected, DiffError, DiffReport, Op, Risk};
pub use normalize::detect;

use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

fn serializer() -> serde_wasm_bindgen::Serializer {
    // Plain objects rather than `Map`s, so the TS side gets ordinary JSON shapes.
    serde_wasm_bindgen::Serializer::json_compatible()
}

fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serializer())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

fn err_to_js(err: DiffError) -> JsValue {
    err.serialize(&serializer())
        .unwrap_or_else(|_| JsValue::from_str(&err.to_string()))
}

/// Diff two documents. Resolves to a `DiffReport`; rejects with a
/// `{ error, line?, column?, side? }` object on malformed input.
#[wasm_bindgen]
pub fn diff(before: &str, after: &str) -> Result<JsValue, JsValue> {
    let report = diff_documents(before, after).map_err(err_to_js)?;
    to_js(&report)
}

/// Identify a single document: kind, syntax, resource count.
#[wasm_bindgen]
pub fn detect_document(doc: &str) -> Result<JsValue, JsValue> {
    let detected = detect(doc).map_err(err_to_js)?;
    to_js(&detected)
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
