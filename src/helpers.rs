use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::Command;

// !!! TODO: This tool is not working with universal binaries! The reason is
// that when we run "otool -l" or "otool -L" commands, we assume the output is
// structured specifically for one arctitecture. But it turns out that if we
// dealing with universal binaries, actually there are multiple dynamic library
// groups or multiple LC_RPATH commands. So for now this tool is not expected
// to work with universal binaries.

#[derive(Debug, PartialEq)]
pub enum BinType<'a> {
    Executable(&'a Path),
    Dylib(&'a Path),
    StaticLib(&'a Path),
    ObjectFile(&'a Path),
    UBinary(&'a Path),
    Unrecognized(&'a Path),
}

enum RPath<'a> {
    LoaderPath(&'a str),
    ExecutablePath(&'a str),
    Absolute(&'a str),
    Unrecongized(&'a str),
}

pub fn get_load_commands(file_path: &Path) -> Result<String> {
    let output = Command::new("otool")
        .arg("-l")
        .arg(file_path)
        .output()
        .context("Error while running otool for load commands")?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn get_shared_libs(file_path: &Path) -> Result<String> {
    let output = Command::new("otool")
        .arg("-L")
        .arg(file_path)
        .output()
        .context("Error while running otool for shared libraries")?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// This function get all rpaths entries for a binary, convert to an absolute
// path, check if the path is valid and return the first valid one. If there
// is no, then returns Err.
pub fn canonicalize_rpath(binary_path: &Path, rpath_install_name: &str) -> Result<PathBuf> {
    let rpaths = get_rpaths(binary_path, false)?;
    log::debug!("All rpaths: {:?}", rpaths);
    let Some(binary_folder_path) = binary_path.parent() else {
        return Err(anyhow!(
            "Error while getting parent folder of: {}",
            binary_path.display()
        ));
    };
    for rpath in rpaths {
        let abs_rpath = binary_folder_path.join(rpath);
        if !abs_rpath.exists() {
            log::debug!(
                "Rpath canonicalized but directory not exists: {}",
                abs_rpath.display()
            );
            continue;
        }

        let base_install_name = remove_rpath_prefix(rpath_install_name)?;
        let abs_lib_path = abs_rpath.join(base_install_name);
        log::debug!("Absolute path of library: {}", abs_lib_path.display());

        if !abs_lib_path.exists() {
            log::debug!("Library not exists: {}", abs_lib_path.display());
            continue;
        }
        log::debug!("Library exists: {}", abs_lib_path.display());
        return Ok(abs_lib_path);
    }
    Err(anyhow!(
        "Canonicalization failed for: {},\nRpath: {}",
        binary_path.display(),
        rpath_install_name,
    ))
}

pub fn get_rpaths(file_path: &Path, with_prefix: bool) -> Result<Vec<String>> {
    log::debug!("Searching @rpath values for: {}", file_path.display());

    let output = get_load_commands(file_path)?;
    let mut rpath_list: Vec<String> = vec![];

    const LINES_TO_SEARCH: i32 = 3;
    let mut counter = 0;

    for line in output.lines() {
        let line = line.trim();

        if line.contains("LC_RPATH") {
            counter = LINES_TO_SEARCH;
            continue;
        }

        if counter > 0 {
            if line.starts_with("path") {
                if let Some(rpath) = line.split_whitespace().nth(1) {
                    log::debug!("@rpath value found: {}", rpath);
                    let rpath = match check_rpath(rpath) {
                        RPath::LoaderPath(rp) => {
                            if with_prefix {
                                rp
                            } else {
                                remove_loader_path_prefix(rp)?
                            }
                        }
                        RPath::ExecutablePath(rp) => {
                            if with_prefix {
                                rp
                            } else {
                                remove_executable_path_prefix(rp)?
                            }
                        }
                        RPath::Absolute(rp) => {
                            log::debug!(
                                "@rpath value is an absolute system path: {}\nskipping...",
                                rp,
                            );
                            continue;
                        }
                        RPath::Unrecongized(rp) => {
                            log::debug!("Unreconized @rpath value: {}", rp);
                            continue;
                        }
                    };
                    rpath_list.push(rpath.to_string());
                }
            }
            counter -= 1;
        }
    }
    if rpath_list.is_empty() {
        log::error!("No rpath found in: {}", file_path.display());
        return Err(anyhow!("No rpath found in: {}", file_path.display()));
    }
    Ok(rpath_list)
}

fn remove_prefix<'a>(value: &'a str, prefix: &'a str) -> Result<&'a str> {
    if value == &prefix[..prefix.len() - 1] {
        return Ok(".");
    }
    if !value.starts_with(prefix) {
        log::error!("{} not starts with <{}>", value, prefix);
        return Err(anyhow!("No <{}> prefix in: {}", prefix, value));
    }
    let Some(stripped) = value.strip_prefix(prefix) else {
        log::error!("<{}> could not stripped from: {}", prefix, value);
        return Err(anyhow!(
            "Error while stripping prefix <{}> for: {}",
            prefix,
            value
        ));
    };
    log::debug!("Rpath stripped: {}", stripped);

    Ok(stripped)
}

fn remove_loader_path_prefix(value: &str) -> Result<&str> {
    let prefix = "@loader_path/";
    remove_prefix(value, prefix)
}

fn check_rpath(rpath: &'_ str) -> RPath<'_> {
    if rpath.starts_with("@loader_path") {
        RPath::LoaderPath(rpath)
    } else if rpath.starts_with("@executable_path") {
        RPath::ExecutablePath(rpath)
    } else if rpath.starts_with("/") {
        RPath::Absolute(rpath)
    } else {
        RPath::Unrecongized(rpath)
    }
}

fn remove_executable_path_prefix(value: &str) -> Result<&str> {
    let prefix = "@executable_path/";
    remove_prefix(value, prefix)
}

fn remove_rpath_prefix(value: &str) -> Result<&str> {
    let prefix = "@rpath/";
    remove_prefix(value, prefix)
}

// [-] TEST: Not tested yet.
// [+] TODO: Not implemented yet.
pub fn get_id(file_path: &Path) -> Result<String> {
    let file_path = match check_input_file(file_path)? {
        BinType::Dylib(fp) => fp,
        _ => {
            return Err(anyhow!(
                "File not a dynamic library: {}",
                file_path.display()
            ));
        }
    };

    let output = Command::new("otool").arg("-D").arg(file_path).output()?;

    // !!! TODO:
    // This implementation is not compatible with universal binaries.
    // It is assumed that we have two lines like:
    //      foo.dylib:
    //      @rpath/foo.dylib
    // However in universal binaries there could be more than two such as:
    //      foobar (architecture x86_64):
    //      @rpath/foobar.framework/Versions/A/foobar
    //      foobar (architecture arm64):
    //      @rpath/foobar.framework/Versions/A/foobar

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(id) = stdout.lines().nth(1) else {
        return Err(anyhow!(
            "No id found in the library: {}",
            file_path.display()
        ));
    };

    log::debug!("Id found in: {}\nId: {}", file_path.display(), id);
    Ok(id.to_string())
}

pub fn fix_id(file_path: &Path, id: &str) -> Result<()> {
    let output = Command::new("install_name_tool")
        .arg("-id")
        .arg(id)
        .arg(file_path)
        .output()?;
    if output.status.success() {
        log::info!("Id is set for: {}", file_path.display());
        log::info!("Id is set to: {}", id);
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "Error while setting id for: {}\n{}",
            file_path.display(),
            err
        ));
    }
    Ok(())
}

// File path should be the path of the new/coppied/dublicate binary path.
// Otherwise this command will change the binary already in use and this
// most probably breaks the dependency cycle.
pub fn fix_install_name(file_path: &Path, old: &str, new: &str) -> Result<()> {
    let output = Command::new("install_name_tool")
        .arg("-change")
        .arg(old)
        .arg(new)
        .arg(file_path)
        .output()?;
    if output.status.success() {
        log::info!("install_name changed for: {}", file_path.display());
        log::info!("Install name was: {}", old);
        log::info!("Install name is now: {}", old);
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "Error while changing install name for: {}\n{}",
            file_path.display(),
            err
        ));
    }
    Ok(())
}

// [-] TODO: Not implemented yet.
// Considering to add a support to remove old rpaths unnecessary.
// pub fn remove_rpath(file_path: &Path, old_rpath: &str) -> Result<()> {
//     todo!()
// }

pub fn add_rpath(file_path: &Path, new_rpath: &str) -> Result<()> {
    let output = Command::new("install_name_tool")
        .arg("-add_rpath")
        .arg(new_rpath)
        .arg(file_path)
        .output()?;
    if output.status.success() {
        log::info!("Rpath added to: {}", file_path.display());
        log::info!("New rpath is now: {}", new_rpath);
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        if err.contains("file already has LC_RPATH") {
            log::debug!("Already has rpath: {}", file_path.display());
            return Ok(());
        }
        return Err(anyhow!(
            "Error while adding rpath to: {}\n{}",
            file_path.display(),
            err
        ));
    }
    Ok(())
}

// DONE
pub fn sign_binary(file_path: &Path) -> Result<()> {
    let output = Command::new("codesign")
        .arg("--force")
        .arg("--sign")
        .arg("-")
        .arg(file_path)
        .output()?;
    if output.status.success() {
        log::info!("Binary singned successfully: {}", file_path.display());
        return Ok(());
    } else {
        log::error!(
            "Binary could not singned successfully: {}",
            file_path.display()
        );
        return Err(anyhow!(
            "Error while signing binary: {}",
            file_path.display()
        ));
    }
}

// DONE
pub fn check_input_file(file_path: &'_ Path) -> Result<BinType<'_>> {
    if !file_path.exists() {
        return Err(anyhow!(
            "Input file is not a valid path: {}",
            file_path.display()
        ));
    }

    let output = Command::new("file").arg(file_path).output()?;

    if !output.status.success() {
        return Err(anyhow!("File command failed on: {}", file_path.display()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("Mach-O") && stdout.contains("executable") {
        log::debug!("Input file is an executable: {}", file_path.display());
        Ok(BinType::Executable(file_path))
    } else if stdout.contains("Mach-O") && stdout.contains("dynamically linked shared") {
        log::debug!("Input file is a dynamic library: {}", file_path.display());
        Ok(BinType::Dylib(file_path))
    } else if stdout.contains("ar archive") {
        log::debug!("Input file is a static library: {}", file_path.display());
        Ok(BinType::StaticLib(file_path))
    } else if stdout.contains("Mach-O") && stdout.contains("object") {
        log::debug!("Input file is an object file: {}", file_path.display());
        Ok(BinType::ObjectFile(file_path))
    } else if stdout.contains("Mach-O") && stdout.contains("universal") {
        log::debug!(
            "Input file is a universal binary file: {}",
            file_path.display()
        );
        Ok(BinType::UBinary(file_path))
    } else {
        log::error!("Input file not recognized: {}", file_path.display());
        log::error!("Stdout: {}", stdout);
        log::debug!(
            "File not recognized as a valid library or an executable: {}",
            file_path.display()
        );
        Ok(BinType::Unrecognized(file_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXECUTABLE_BINARY: &str = "poppler/pdftoppm";
    const DYLIB_BINARY_1: &str = "poppler/libs/libassuan.9.dylib";
    const DYLIB_BINARY_2: &str = "poppler/libs/libfreetype.6.dylib";
    const STATICLIB_BINARY: &str = "libzstd.a";
    const INVALID_BINARY: &str = "poppler/libs/libfoobarxyz.dylib";

    fn _get_resource_path(relative: &str) -> PathBuf {
        let resources_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_resources");
        resources_path.join(relative)
    }

    mod tests_check_input_file {
        use super::*;
        #[test]
        fn test_check_input_file_1() {
            let file = _get_resource_path(EXECUTABLE_BINARY);
            let res = check_input_file(&file);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), BinType::Executable(&file));
        }

        #[test]
        fn test_check_input_file_2() {
            let file = _get_resource_path(DYLIB_BINARY_1);
            let res = check_input_file(&file);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), BinType::Dylib(&file));
        }

        #[test]
        fn test_check_input_file_3() {
            let file = _get_resource_path(STATICLIB_BINARY);
            let res = check_input_file(&file);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), BinType::StaticLib(&file));
        }
        #[test]
        fn test_check_input_file_4() {
            let file = PathBuf::from(INVALID_BINARY);
            let res = check_input_file(&file);
            assert!(res.is_err());
        }
    }

    mod tests_sign_binary {
        use super::*;
        #[test]
        fn test_sign_binary_1() {
            let file = _get_resource_path(EXECUTABLE_BINARY);
            let res = sign_binary(&file);
            assert!(res.is_ok());
        }
        #[test]
        fn test_sign_binary_2() {
            let file = _get_resource_path(DYLIB_BINARY_1);
            let res = sign_binary(&file);
            assert!(res.is_ok());
        }
        #[test]
        fn test_sign_binary_3() {
            let file = _get_resource_path(INVALID_BINARY);
            let res = sign_binary(&file);
            assert!(res.is_err());
        }
    }

    mod tests_get_rpaths {
        use super::*;
        #[test]
        fn test_get_rpats_1() {
            let file = _get_resource_path(EXECUTABLE_BINARY);
            let mut expected: Vec<String> = Vec::new();
            expected.push("../lib".to_string());
            expected.push("libs".to_string());
            let res = get_rpaths(&file, false);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), expected);
        }

        #[test]
        fn test_get_rpats_2() {
            let file = _get_resource_path(EXECUTABLE_BINARY);
            let mut expected: Vec<String> = Vec::new();
            expected.push("@loader_path/../lib".to_string());
            expected.push("@loader_path/libs".to_string());
            let res = get_rpaths(&file, true);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), expected);
        }

        #[test]
        fn test_get_rpats_3() {
            let file = _get_resource_path(DYLIB_BINARY_1);
            let mut expected: Vec<String> = Vec::new();
            expected.push("@loader_path".to_string());
            let res = get_rpaths(&file, true);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), expected);
        }
    }

    mod tests_canonicalize_rpath {
        use super::*;

        #[test]
        fn test_canonicalize_rpath_1() {
            let file = _get_resource_path(EXECUTABLE_BINARY);
            let rpath_install_name = "@rpath/libpoppler.154.0.0.dylib";
            let expected = _get_resource_path("poppler/libs/libpoppler.154.0.0.dylib");
            let res = canonicalize_rpath(&file, rpath_install_name);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), expected);
        }
        #[test]
        fn test_canonicalize_rpath_2() {
            let file = _get_resource_path(DYLIB_BINARY_1);
            let rpath_install_name = "@rpath/libassuan.9.dylib";
            let expected = _get_resource_path("poppler/libs/libassuan.9.dylib");
            let res = canonicalize_rpath(&file, rpath_install_name);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), expected);
        }
    }

    mod test_get_id {
        use super::*;

        #[test]
        fn test_get_id_1() {
            let file = _get_resource_path(DYLIB_BINARY_1);
            let expected = "@rpath/libassuan.9.dylib";
            let res = get_id(&file);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), expected);
        }
        #[test]
        fn test_get_id_2() {
            let file = _get_resource_path(DYLIB_BINARY_2);
            let expected = "@rpath/libfreetype.6.dylib";
            let res = get_id(&file);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), expected);
        }
    }
}
