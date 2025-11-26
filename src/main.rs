use crate::{
    helpers::{BinType, check_input_file},
    model::Binary,
};
use anyhow::anyhow;
use std::path::PathBuf;

pub mod helpers;
pub mod model;

fn main() -> anyhow::Result<()> {
    const USAGE: &str = r#"
    USAGE:
    bbundler [--binary_path | -bp]  [/binary/path] [--out_dir | -o] [/output/path]
    "#;

    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "DEBUG") };
    }
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        return Err(anyhow!("Missing arguments! {}", USAGE));
    }

    let mut args_iter = args.into_iter().skip(1);

    let bp_flag = args_iter
        .next()
        .ok_or_else(|| anyhow!("Missing argument: [--binary_path | -bp] {}", USAGE))?;

    let binary_path = args_iter
        .next()
        .ok_or_else(|| anyhow!("Missing argument: /binary/path {}", USAGE))?;

    let out_flag = args_iter
        .next()
        .ok_or_else(|| anyhow!("Missing argument: [--out_dir | -o] {}", USAGE))?;

    let out_dir = args_iter
        .next()
        .ok_or_else(|| anyhow!("Missing argument: /output/path {}", USAGE))?;

    if bp_flag != "--binary_path" && bp_flag != "-bp" {
        return Err(anyhow!("Missing argument: [--binary_path | -bp] {}", USAGE));
    }

    if out_flag != "--out_dir" && out_flag != "-o" {
        return Err(anyhow!("Missing argument: [--out_dir | -o] {}", USAGE));
    }

    let binary_path = std::path::PathBuf::from(binary_path);
    let out_dir = PathBuf::from(out_dir);

    if out_dir.exists() && out_dir.is_file() {
        return Err(anyhow!(
            "Output path is a file!\nOutput path must be a folder."
        ));
    }

    if !out_dir.exists() {
        let _ = std::fs::create_dir_all(&out_dir)?;
    }

    let res = check_input_file(&binary_path)?;

    let mut binary = match res {
        BinType::Executable(_) => Binary::new(binary_path, true, true)?,
        BinType::Dylib(_) => Binary::new(binary_path, false, true)?,
        _ => return Err(anyhow!("Input file not recognized!")),
    };
    binary.set_dest_folder(&out_dir);
    binary.run()?;
    Ok(())
}

// TODOS:
// [-] Add much better args parsing. Considering crates like `clap`?.
// [-] Improve loggings & the logic for runtime debug and info messages.
// [-] Add an option to cli args for logging level selection.
// [-] Add universal binary support.
// [-] Complete tests.
