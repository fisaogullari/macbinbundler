use crate::helpers::*;
use anyhow::{Result, anyhow};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct Binary {
    file_path: PathBuf,
    is_executable: bool,
    is_base: bool,
    install_name_old: Option<String>,
    install_name_id: Option<String>,
    dest_folder_path: Option<PathBuf>,
    dest_file_path: Option<PathBuf>,
    libs_path: Option<PathBuf>,
    rpath: Option<String>,
    libs: Vec<Binary>,
}

impl Binary {
    pub fn new(file_path: PathBuf, is_executable: bool, is_base: bool) -> Result<Self> {
        if !file_path.is_file() || !file_path.exists() {
            return Err(anyhow!("Path not exist or a file: {}", file_path.display()));
        }

        Ok(Binary {
            file_path,
            is_executable,
            is_base,
            ..Default::default()
        })
    }

    pub fn run(&mut self, dest_folder: &Path, libs_path: Option<&Path>) -> Result<()> {
        let mut libs_checked = HashSet::<PathBuf>::new();

        self.get_libs(&mut libs_checked)?;
        self.resolve_symlinks()?;
        self.change_install_names_as_rpath()?;
        log::trace!("Binary Structure:\n {:#?}", self);
        self.set_libs_path(libs_path);
        self.set_dest_folder(dest_folder);
        self.copy_to_dest()?;
        self.fix_install_names()?;
        self.sign_all()?;

        Ok(())
    }

    fn set_libs_path(&mut self, libs_path: Option<&Path>) {
        if let Some(libs_path) = libs_path {
            self.libs_path = Some(libs_path.to_path_buf());
        } else {
            self.libs_path = Some(PathBuf::from("libs"))
        }
        for lib in &mut self.libs {
            let _ = lib.set_libs_path(libs_path);
        }
    }

    fn set_dest_folder(&mut self, dest_folder: &Path) {
        if self.is_base {
            self.dest_folder_path = Some(dest_folder.to_path_buf());
        } else {
            if let Some(ref libs_path) = self.libs_path {
                self.dest_folder_path = Some(dest_folder.join(libs_path));
            } else {
                self.dest_folder_path = Some(dest_folder.join("libs"));
            }
        }

        for lib in &mut self.libs {
            let _ = lib.set_dest_folder(dest_folder);
        }
    }
    // [-] TODO: <@executable_path> should be handled as well.
    fn get_libs(&mut self, libs_checked: &mut HashSet<PathBuf>) -> Result<()> {
        if libs_checked.contains(&self.file_path) {
            log::info!(
                "Library already collected: {}\nSkipping",
                self.file_path.display()
            );
            return Ok(());
        }

        let output = get_shared_libs(&self.file_path)?;
        let mut lines = output.lines().skip(1);

        // If Binary is not an executable, we need to treat first line separately
        // since it is the id of the shared library.
        if !self.is_executable {
            let Some(id) = lines.next() else {
                return Err(anyhow!(
                    "Error while reading the id of: {}",
                    self.file_path.display()
                ));
            };
            let id = id
                .trim()
                .split_whitespace()
                .nth(0)
                .ok_or_else(|| anyhow!("Error while processing line:{}", id))?;
            self.install_name_id = Some(id.to_string());
        }

        for line in lines {
            let line = line
                .trim()
                .split_whitespace()
                .nth(0)
                .ok_or_else(|| anyhow!("Error while processing line: {}", line))?;
            log::debug!("Processing library: {}", line);

            if line.starts_with("/usr/lib") || line.starts_with("/System/Library") {
                log::debug!("Skipping system library: {}", line);
                continue;
            }

            if line.starts_with("@rpath") {
                let abs_path = canonicalize_rpath(&self.file_path, line)?;
                let mut lib = Binary::new(abs_path, false, false)?;
                lib.install_name_old = Some(line.to_string());
                self.libs.push(lib);

                continue;
            }

            if Path::new(line).is_absolute() {
                let mut lib = Binary::new(PathBuf::from(line), false, false)?;
                lib.install_name_old = Some(line.to_string());
                self.libs.push(lib);
                continue;
            }

            panic!("Unrecognized library: {}", line);
        }

        libs_checked.insert(self.file_path.clone());

        for lib in &mut self.libs {
            let _ = lib.get_libs(libs_checked)?;
        }

        Ok(())
    }

    fn resolve_symlinks(&mut self) -> Result<()> {
        log::trace!("Checking if symlink for: {}", self.file_path.display());
        if self.file_path.is_symlink() {
            log::debug!("Symlink found for: {}", self.file_path.display());
            let real_path = self.file_path.canonicalize()?;
            self.file_path = real_path;
        }

        for lib in &mut self.libs {
            let _ = lib.resolve_symlinks()?;
        }

        Ok(())
    }

    fn copy_to_dest(&mut self) -> Result<()> {
        log::debug!("Copying: {}", self.file_path.display());
        let Some(ref dest_folder_path) = self.dest_folder_path else {
            return Err(anyhow!(
                "Error while retrieving destionation path of: {}",
                self.file_path.display()
            ));
        };
        log::debug!("Destination folder path: {}", dest_folder_path.display());

        log::debug!("Creating folder: {}", dest_folder_path.display());
        std::fs::create_dir_all(dest_folder_path)?;

        let Some(file_name) = self.file_path.file_name() else {
            return Err(anyhow!(
                "Error while retrieving file name of: {}",
                self.file_path.display()
            ));
        };

        let dest_file_path = dest_folder_path.join(file_name);
        log::debug!("Destination full path: {}", dest_file_path.display());
        self.dest_file_path = Some(dest_file_path.clone());

        if !dest_file_path.exists() {
            let _ = std::fs::copy(&self.file_path, &dest_file_path)?;
        }

        for lib in &mut self.libs {
            let _ = lib.copy_to_dest()?;
        }
        Ok(())
    }

    fn fix_install_names(&self) -> Result<()> {
        let Some(ref dest_file_path) = self.dest_file_path else {
            return Err(anyhow!(
                "No destination file path found for: {}",
                self.file_path.display()
            ));
        };
        if !self.is_executable {
            let Some(ref parent_rpath) = self.rpath else {
                return Err(anyhow!(
                    "No rpath path found for: {}",
                    self.file_path.display()
                ));
            };
            fix_id(dest_file_path, parent_rpath)?;
        }

        if self.is_base {
            let Some(ref libs_path) = self.libs_path else {
                return Err(anyhow!("No path found for libraries"));
            };
            let rpath = PathBuf::from("@loader_path").join(libs_path);
            let _ = add_rpath(dest_file_path, &rpath)?;
        } else {
            let _ = add_rpath(dest_file_path, Path::new("@loader_path"))?;
        }

        for lib in &self.libs {
            if let Some(ref child_rpath) = lib.rpath {
                let Some(ref old_install_name) = lib.install_name_old else {
                    return Err(anyhow!(
                        "No old install name found for: {}",
                        lib.file_path.display()
                    ));
                };
                fix_install_name(dest_file_path, old_install_name, child_rpath)?;
            };
        }

        for lib in &self.libs {
            let _ = lib.fix_install_names()?;
        }
        Ok(())
    }

    fn change_install_names_as_rpath(&mut self) -> Result<()> {
        if !self.is_executable {
            let Some(rpath) = self
                .file_path
                .file_name()
                .map(|f_name| format!("@rpath/{}", f_name.display()))
            else {
                return Err(anyhow!(
                    "Error while calculating rpath for: {}",
                    self.file_path.display()
                ));
            };
            log::debug!("Rpath calculated for: {}", self.file_path.display());
            log::debug!("Rpath calculated to: {}", rpath);
            self.rpath = Some(rpath);
        }

        for lib in &mut self.libs {
            let _ = lib.change_install_names_as_rpath()?;
        }
        Ok(())
    }

    // Signing method based on walking in file system
    // NOT USED
    #[allow(dead_code)]
    fn sign_all_alternative(&self) -> Result<()> {
        let Some(ref base_dest_path) = self.dest_file_path else {
            return Err(anyhow!(
                "Error while retrieving destionation path of: {}",
                self.file_path.display()
            ));
        };

        let Some(ref libs_path) = self.libs_path else {
            return Err(anyhow!(
                "Error while retrieving libraries path of: {}",
                self.file_path.display()
            ));
        };

        sign_binary(base_dest_path)?;

        let Some(ref dest_folder_path) = self.dest_folder_path else {
            return Err(anyhow!(
                "Error while retrieving destionation folder path of: {}",
                self.file_path.display()
            ));
        };

        for lib in std::fs::read_dir(dest_folder_path.join(libs_path))? {
            let lib_path = lib?.path();
            match check_input_file(&lib_path)? {
                BinType::Dylib(_) => sign_binary(&lib_path)?,
                _ => log::debug!("Not dynamic library, skipping signing..."),
            };
        }
        Ok(())
    }

    fn sign_all(&self) -> Result<()> {
        let Some(ref dest_path) = self.dest_file_path else {
            return Err(anyhow!(
                "Error while retrieving destionation path of: {}",
                self.file_path.display()
            ));
        };
        sign_binary(dest_path)?;

        for lib in &self.libs {
            let _ = lib.sign_all()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_binary() {
        let binary = Binary::default();
        println!("Default initialization:\n{:#?}", binary);
        assert!(!binary.is_executable);
    }
}
