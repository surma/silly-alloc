use std::{
    process::Command,
    string::{String, ToString},
    vec::Vec,
};

use anyhow::Result;

use wasmtime::*;

#[test]
fn run() -> Result<()> {
    Command::new("cargo")
        .args(&[
            "build",
            "--target=wasm32-unknown-unknown",
            "--release",
            "--package",
            "e2e",
        ])
        .spawn()?
        .wait()?;

    let engine = Engine::default();
    let module = Module::from_file(&engine, "./target/wasm32-unknown-unknown/release/e2e.wasm")?;
    let mut store = Store::new(&engine, 0);
    // module
    let test_func_names: Vec<String> = module
        .exports()
        .filter(|export| matches!(export.ty(), ExternType::Func(_)))
        .map(|export| export.name().to_string())
        .collect();

    for test_func_name in test_func_names {
        let instance = Instance::new(&mut store, &module, &[])?;
        let f = instance.get_typed_func::<(), ()>(&mut store, &test_func_name)?;
        f.call(&mut store, ())?;
    }
    Ok(())
}
