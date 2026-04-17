//! .agent ZIP package loader + manifest validation

use rollball_core::AgentManifest;
use std::path::Path;

/// Load .agent package
pub fn load_package(package_path: &Path) -> Result<LoadedPackage, String> {
    // TODO: Implement ZIP extraction and manifest validation
    unimplemented!()
}

/// Loaded package information
pub struct LoadedPackage {
    pub manifest: AgentManifest,
    pub package_dir: std::path::PathBuf,
}
