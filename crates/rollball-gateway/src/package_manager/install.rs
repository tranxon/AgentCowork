//! .agent package installation

use std::path::Path;

/// Install a .agent package
pub fn install_package(package_path: &Path, install_dir: &Path) -> Result<(), String> {
    // TODO: Implement installation (ZIP extract + signature verify + manifest validate)
    unimplemented!()
}
