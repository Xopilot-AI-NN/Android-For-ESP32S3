#[path = "../../platform/tjpgd/build_support.rs"]
mod jpeg;
fn main() {
    jpeg::build(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../platform/tjpgd"));
}
