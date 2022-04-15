use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=resources");

    assert!(Command::new("blueprint-compiler")
        .args([
            "batch-compile",
            "resources",
            "resources",
            "resources/accuracy.blp",
            "resources/combo.blp",
            "resources/window.blp"
        ])
        .status()
        .unwrap()
        .success());

    gio::compile_resources(
        "resources",
        "resources/resources.gresource.xml",
        "compiled.gresource",
    );
}
