use std::{
    fs,
    process::Command,
    string::{String, ToString},
    sync::Once,
    vec::Vec,
};

use anyhow::Result;
use tinytemplate::{self, TinyTemplate};
use wasmtime::*;

use serde::Serialize;

#[derive(Serialize)]
struct TemplateData<'a> {
    name: &'a str,
}

impl<'a> TemplateData<'a> {
    fn new<T: AsRef<str>>(v: &'a T) -> Self {
        Self { name: v.as_ref() }
    }
}

fn render_cargo(data: TemplateData) -> Result<()> {
    static TT_INIT: Once = Once::new();
    static mut TT: Option<TinyTemplate> = None;
    TT_INIT.call_once(|| unsafe {
        TT = Some(TinyTemplate::new());
        let tpl = Box::new(String::from_utf8(fs::read("./e2e/Cargo.toml.tpl").unwrap()).unwrap());
        TT.as_mut()
            .unwrap()
            .add_template("cargotoml", Box::leak(tpl).as_str())
            .unwrap();
    });
    let cargo_contents = unsafe { TT.as_mut().unwrap() }.render("cargotoml", &data)?;
    fs::write("./e2e/Cargo.toml", &cargo_contents)?;
    Ok(())
}

#[test]
fn run() -> Result<()> {
    let tests = fs::read_dir("./e2e")?.flat_map(|entry| {
        let Ok(entry) = entry else {return None};
        let filename = entry.file_name();
        let filename = filename.to_str()?;
        if entry.metadata().ok()?.is_file() && filename.ends_with(".rs") {
            Some(filename.to_string())
        } else {
            None
        }
    });

    for name in tests {
        render_cargo(TemplateData::new(&name))?;

        Command::new("cargo")
            .args(&["build", "--target=wasm32-unknown-unknown", "--release"])
            .current_dir("e2e")
            .spawn()?
            .wait()?;

        let engine = Engine::default();
        let module =
            Module::from_file(&engine, "./target/wasm32-unknown-unknown/release/e2e.wasm")?;
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
    }
    render_cargo(TemplateData { name: "dummy.rs" })?;
    Ok(())
}
