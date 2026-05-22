use tract_onnx::prelude::*;

fn main() {
    let path = "src-tauri/models/arcface.onnx";
    let model = tract_onnx::onnx().model_for_path(path).expect("load");
    let inputs = model.input_outlets().expect("inputs");
    for outlet in inputs {
        let fact = model.outlet_fact(*outlet).expect("fact");
        println!("Input outlet {outlet:?}: fact = {fact:?}");
    }
}
