use std::env::var;
use std::path::PathBuf;

const SHADER: &str = include_str!("src/shader.wgsl");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/shader.wgsl");

    let out_dir = var("OUT_DIR").expect("OUT_DIR environment variable not set");

    let module = naga::front::wgsl::parse_str(SHADER)?;

    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .subgroup_stages(naga::valid::ShaderStages::all())
    .subgroup_operations(naga::valid::SubgroupOperationSet::all())
    .validate(&module)?;

    let blob = postcard::to_stdvec(&module)?;

    let mut path = PathBuf::from(out_dir);
    path.push("shader.naga");
    std::fs::write(path, blob)?;

    Ok(())
}
