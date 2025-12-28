use std::fs;
use std::path::Path;
use std::collections::{HashSet, HashMap};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct CargoToml {
    dependencies: Option<toml::Table>,
    #[serde(rename = "dev-dependencies")]
    dev_dependencies: Option<toml::Table>,
}

#[derive(Deserialize)]
struct PackageJson {
    dependencies: Option<serde_json::Map<String, Value>>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: Option<serde_json::Map<String, Value>>,
}

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

pub struct ManifestResult {
    pub externals: HashSet<String>,
    pub aliases: HashMap<String, String>,
}

pub fn scan_manifests(path: &Path) -> ManifestResult {
    let mut result = ManifestResult {
        externals: HashSet::new(),
        aliases: HashMap::new(),
    };

    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

    if filename == "package.json" {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(json) = serde_json::from_str::<PackageJson>(&content) {
                if let Some(deps) = json.dependencies {
                    result.externals.extend(deps.keys().cloned());
                }
                if let Some(dev_deps) = json.dev_dependencies {
                    result.externals.extend(dev_deps.keys().cloned());
                }
            }
        }
    } else if filename == "Cargo.toml" {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(toml_data) = toml::from_str::<CargoToml>(&content) {
                if let Some(deps) = toml_data.dependencies {
                    result.externals.extend(deps.keys().cloned());
                }
                if let Some(dev_deps) = toml_data.dev_dependencies {
                    result.externals.extend(dev_deps.keys().cloned());
                }
            }
        }
    } else if filename == "requirements.txt" {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') { continue; }
                // Handle version specifiers like "numpy>=1.20" or "requests==2.0"
                let name = trimmed.split(|c| c == '=' || c == '<' || c == '>').next().unwrap_or("").trim();
                if !name.is_empty() {
                    result.externals.insert(name.to_string());
                }
            }
        }
    } else if filename == "pyproject.toml" {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(toml_data) = toml::from_str::<PyProjectToml>(&content) {
                if let Some(tool) = toml_data.tool {
                    if let Some(poetry) = tool.poetry {
                        if let Some(deps) = poetry.dependencies {
                            result.externals.extend(deps.keys().cloned());
                        }
                    }
                }
            }
        }
    } else if filename == "tsconfig.json" || filename == "jsconfig.json" {
        if let Ok(content) = fs::read_to_string(path) {
            // tsconfig.json is often "JSONC" (JSON with comments), which standard serde_json fails on.
            // We perform a very basic stripping of lines that start with // to handle the most common case.
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
        }
    }

    result
}