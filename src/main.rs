use crate::{
    helpers::{BinType, check_input_file},
    model::Binary,
};
use anyhow::anyhow;
use std::path::PathBuf;

pub mod cli;
pub mod helpers;
pub mod model;

fn main() -> anyhow::Result<()> {
    let cli = cli::init_cli();

    if let Some(log_level) = cli.get_one::<String>("LOG_LEVEL") {
        unsafe { std::env::set_var("RUST_LOG", log_level) };
    } else {
        // Actually we don't need this else block since <log_level>
        // has a default value of INFO. However, setting it
        // to INFO is a good idea for safety.
        unsafe { std::env::set_var("RUST_LOG", "INFO") };
    };

    env_logger::init();

    let Some(binary_path) = cli.get_one::<String>("BINARY_PATH") else {
        // TODO: Add the github link to the error.
        return Err(anyhow!(
            "Internal Logic Error: Failed to retrieve required argument 'BINARY_PATH' after argument validation. Please report an issue."
        ));
    };

    let Some(bundle_path) = cli.get_one::<String>("BUNDLE_PATH") else {
        // TODO: Add the github link to the error.
        return Err(anyhow!(
            "Internal Logic Error: Failed to retrieve required argument 'BUNDLE_PATH' after argument validation. Please report an issue."
        ));
    };

    let binary_path = PathBuf::from(binary_path);
    let bundle_path = PathBuf::from(bundle_path);

    if bundle_path.exists() && bundle_path.is_file() {
        return Err(anyhow!(
            "Output path is a file!\nOutput path must be a folder."
        ));
    }

    if !bundle_path.exists() {
        let _ = std::fs::create_dir_all(&bundle_path)?;
    }

    let res = check_input_file(&binary_path)?;

    let mut binary = match res {
        BinType::Executable(_) => Binary::new(binary_path, true, true)?,
        BinType::Dylib(_) => Binary::new(binary_path, false, true)?,
        _ => return Err(anyhow!("Input file not recognized!")),
    };
    binary.set_dest_folder(&bundle_path);
    binary.run()?;
    Ok(())
}

// TODOS:
// [-] Add much better args parsing. Considering crates like `clap`?.
// [-] Improve loggings & the logic for runtime debug and info messages.
// [-] Add an option to cli args for logging level selection.
// [-] Add universal binary support.
// [-] Complete tests.
