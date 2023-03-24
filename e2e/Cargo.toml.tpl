[package]
name = "e2e"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]
path = "{name}"

[dependencies]
silly-alloc = \{ path = "../" }
