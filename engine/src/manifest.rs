use std::fs;
use std::path::Path;
use std::collections::HashSet;
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

pub fn scan_manifests(path: &Path) -> HashSet<String> {
    let mut externals = HashSet::new();
    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

    if filename == "package.json" {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(json) = serde_json::from_str::<PackageJson>(&content) {
                if let Some(deps) = json.dependencies {
                    externals.extend(deps.keys().cloned());
                }
                if let Some(dev_deps) = json.dev_dependencies {
                    externals.extend(dev_deps.keys().cloned());
                }
            }
        }
    } else if filename == "Cargo.toml" {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(toml_data) = toml::from_str::<CargoToml>(&content) {
                if let Some(deps) = toml_data.dependencies {
                    externals.extend(deps.keys().cloned());
                }
                if let Some(dev_deps) = toml_data.dev_dependencies {
                    externals.extend(dev_deps.keys().cloned());
                }
            }
        }
    } else if filename == "requirements.txt" {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') { continue; }
                let name = trimmed.split(|c| c == '=' || c == '<' || c == '>').next().unwrap_or("").trim();
                if !name.is_empty() {
                    externals.insert(name.to_string());
                }
            }
        }
    } else if filename == "pyproject.toml" {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(toml_data) = toml::from_str::<PyProjectToml>(&content) {
                if let Some(tool) = toml_data.tool {
                    if let Some(poetry) = tool.poetry {
                        if let Some(deps) = poetry.dependencies {
                            externals.extend(deps.keys().cloned());
                        }
                    }
                }
            }
        }
    }

    externals
}