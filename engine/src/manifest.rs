use std::fs;
use std::path::Path;
use std::collections::{HashSet, HashMap};
use serde::Deserialize;
use serde_json::Value;

// --- Cargo.toml Definitions ---

#[derive(Deserialize)]
struct CargoToml {
    package: Option<CargoPackage>,
    dependencies: Option<toml::Table>,
    #[serde(rename = "dev-dependencies")]
    dev_dependencies: Option<toml::Table>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
}

// --- package.json Definitions ---

#[derive(Deserialize)]
struct PackageJson {
    name: Option<String>,
    dependencies: Option<serde_json::Map<String, Value>>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: Option<serde_json::Map<String, Value>>,
}

// --- Python Definitions ---

#[derive(Deserialize)]
struct PyProjectToml {
    tool: Option<PyTool>,
}

#[derive(Deserialize)]
struct PyTool {
    poetry: Option<PyPoetry>,
}

#[derive(Deserialize)]
struct PyPoetry {
    dependencies: Option<toml::Table>,
}

// --- TypeScript Config Definitions ---

#[derive(Deserialize)]
struct TsConfig {
    #[serde(rename = "compilerOptions")]
    compiler_options: Option<CompilerOptions>,
}

#[derive(Deserialize)]
struct CompilerOptions {
    #[serde(default)]
    paths: HashMap<String, Vec<String>>,
}

// --- Result Structure ---

pub struct ManifestResult {
    /// The name of the package defined in this manifest (e.g. "@my-org/ui" or "my-crate").
    /// Used for Monorepo/Workspace local resolution.
    pub package_name: Option<String>, 
    /// External libraries used by this package (e.g. "react", "serde").
    pub externals: HashSet<String>,
    /// Path aliases defined in this config (e.g. "@/*" -> "src/*").
    pub aliases: HashMap<String, String>,
}

/// Helper wrapper that reads from disk (useful for tests or standalone usage)
pub fn scan_manifests(path: &Path) -> ManifestResult {
    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    // If read fails, we just treat it as empty content
    let content = fs::read_to_string(path).unwrap_or_default();
    scan_manifest_content(filename, &content)
}

/// Core parsing logic. Accepts content in-memory to avoid double I/O during scanning.
pub fn scan_manifest_content(filename: &str, content: &str) -> ManifestResult {
    let mut result = ManifestResult {
        package_name: None,
        externals: HashSet::new(),
        aliases: HashMap::new(),
    };

    if filename == "package.json" {
        if let Ok(json) = serde_json::from_str::<PackageJson>(content) {
            // 1. Capture Package Name
            result.package_name = json.name;

            // 2. Capture Dependencies
            if let Some(deps) = json.dependencies {
                result.externals.extend(deps.keys().cloned());
            }
            if let Some(dev_deps) = json.dev_dependencies {
                result.externals.extend(dev_deps.keys().cloned());
            }
        }
    } else if filename == "Cargo.toml" {
        if let Ok(toml_data) = toml::from_str::<CargoToml>(content) {
            // 1. Capture Package Name
            if let Some(pkg) = toml_data.package {
                result.package_name = Some(pkg.name);
            }

            // 2. Capture Dependencies
            if let Some(deps) = toml_data.dependencies {
                result.externals.extend(deps.keys().cloned());
            }
            if let Some(dev_deps) = toml_data.dev_dependencies {
                result.externals.extend(dev_deps.keys().cloned());
            }
        }
    } else if filename == "requirements.txt" {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') { continue; }
            // Handle version specifiers like "numpy>=1.20" or "requests==2.0"
            let name = trimmed.split(|c| c == '=' || c == '<' || c == '>').next().unwrap_or("").trim();
            if !name.is_empty() {
                result.externals.insert(name.to_string());
            }
        }
    } else if filename == "pyproject.toml" {
        if let Ok(toml_data) = toml::from_str::<PyProjectToml>(content) {
            if let Some(tool) = toml_data.tool {
                if let Some(poetry) = tool.poetry {
                    if let Some(deps) = poetry.dependencies {
                        result.externals.extend(deps.keys().cloned());
                    }
                }
            }
        }
    } else if filename == "tsconfig.json" || filename == "jsconfig.json" {
        // tsconfig.json is often "JSONC" (JSON with comments).
        // We perform a basic stripping of lines that start with // to handle common cases.
        let clean_content: String = content
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        if let Ok(ts_config) = serde_json::from_str::<TsConfig>(&clean_content) {
            if let Some(opts) = ts_config.compiler_options {
                for (key, values) in opts.paths {
                    if let Some(target) = values.first() {
                        // Normalize the mapping.
                        // Example: "@/*": ["src/*"]  =>  "@/" -> "src/"
                        // Example: "~": ["src"]      =>  "~"  -> "src"
                        let key_clean = key.replace('*', "");
                        let target_clean = target.replace('*', "");
                        
                        if !key_clean.is_empty() && !target_clean.is_empty() {
                            result.aliases.insert(key_clean, target_clean);
                        }
                    }
                }
            }
        }
    } else if filename == "DESCRIPTION" {
        // Simple manual parser for Debian Control format
        let mut in_deps = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }

            // Handle continuation lines (indented)
            if line.starts_with(' ') || line.starts_with('\t') {
                if in_deps {
                    // Extract package name from "    dplyr (>= 1.0),"
                    let pkg = trimmed.split('(').next().unwrap_or(trimmed).replace(',', "").trim().to_string();
                    if !pkg.is_empty() && pkg != "R" {
                        result.externals.insert(pkg);
                    }
                }
                continue;
            }

            // Detect Field Headers
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();

                if key == "Package" {
                    result.package_name = Some(value.trim().to_string());
                    in_deps = false;
                } else if key == "Imports" || key == "Depends" || key == "Suggests" || key == "LinkingTo" {
                    in_deps = true;
                    // Handle inline values: "Imports: dplyr, ggplot2"
                    let deps_line = value.trim();
                    if !deps_line.is_empty() {
                        for part in deps_line.split(',') {
                            let pkg = part.split('(').next().unwrap_or(part).trim().to_string();
                            if !pkg.is_empty() && pkg != "R" {
                                result.externals.insert(pkg);
                            }
                        }
                    }
                } else {
                    in_deps = false;
                }
            }
        }
    }

    result
}