use crate::{
    helpers::{BinType, check_file_type},
    model::Binary,
};
use anyhow::anyhow;
use std::path::{Path, PathBuf};

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
        return Err(anyhow!(
            "Cli Error: Failed to retrieve required argument `BINARY_PATH` after argument validation. Please report an issue on `https://github.com/fisaogullari/macbinbundler`."
        ));
    };

    let Some(output_path) = cli.get_one::<String>("OUTPUT_PATH") else {
        return Err(anyhow!(
            "Cli Error: Failed to retrieve required argument `OUTPUT_PATH` after argument validation. Please report an issue on `https://github.com/fisaogullari/macbinbundler`."
        ));
    };

    let Ok(libs_path) = cli.try_get_one::<String>("LIBS_PATH") else {
        return Err(anyhow!(
            "Cli Error: Failed to retrieve required argument `LIBS_PATH` after argument validation. Please report an issue on `https://github.com/fisaogullari/macbinbundler`."
        ));
    };

    let create_bundle_path = cli.get_flag("CREATE_OUTPUT_PATH");

    let binary_path = PathBuf::from(binary_path);
    let output_path = PathBuf::from(output_path);
    let libs_path = libs_path.map(|s| Path::new(s));

    if output_path.exists() && output_path.is_file() {
        return Err(anyhow!(
            "Output path is a file!\nOutput path must be a folder."
        ));
    }

    if !output_path.exists() {
        if create_bundle_path {
            let _ = std::fs::create_dir_all(&output_path)?;
        } else {
            return Err(anyhow!(
                "Destination path not exist: {}\nPlease make sure it exists or consider using <-c | --create-bundle-path> flag to create folder!",
                output_path.display()
            ));
        }
    }

    let res = check_file_type(&binary_path)?;

    let mut binary = match res {
        BinType::Executable(_) => Binary::new(binary_path, true, true)?,
        BinType::Dylib(_) => Binary::new(binary_path, false, true)?,
        _ => {
            return Err(anyhow!(
                "Input file not recognized!\nMust be an executable or a dynamic library: {}",
                binary_path.display()
            ));
        }
    };

    binary.run(&output_path, libs_path)?;

    Ok(())
}

// TODOS:
// [-] Add much better args parsing. Considering crates like `clap`?.
// [-] Improve loggings & the logic for runtime debug and info messages.
// [-] Add an option to cli args for logging level selection.
// [-] Add universal binary support.
// [-] Complete tests.
