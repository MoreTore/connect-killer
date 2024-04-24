extern crate capnp;
use std::env;

fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("cereal/")
        .file("cereal/log.capnp")
        .file("cereal/car.capnp")
        .file("cereal/custom.capnp")
        .file("cereal/legacy.capnp")
        .file("cereal/maptile.capnp")
        .output_path("src/cereal/")
        .default_parent_module(vec!["cereal".into(),])
        .run()
        .expect("schema compiler command");
}