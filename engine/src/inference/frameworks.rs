use crate::analysis::language::SupportedLanguage;
use crate::inference::InferenceRule;
use crate::models::{FileBoundary, FrameworkHint};
use regex::Regex;
use std::collections::HashMap;

/// The Universal Concepts we want to extract from any framework.
#[derive(Debug, Clone)]
pub enum ConceptType {
    /// A URL Endpoint (e.g., "GET /api/users") -> "route:{method}:{path}"
    Route,
    /// A Database Model/Table -> "model:{name}"
    Model,
    /// A View/Template/Component -> "view:{name}" or "html:tag:{name}"
    View,
    /// A Dependency Injection Token -> "di:{token}"
    DependencyProvider,
    /// A Reference to a DI Token -> "di:{token}" (Usage)
    DependencyConsumer,
}

/// Defines HOW to extract a concept from a specific framework pattern.
#[derive(Debug)]
pub struct ConceptRule {
    /// The concept we are generating
    pub concept: ConceptType,

    /// 1. Trigger: The 'key' in `framework_hints` to look for.
    /// e.g., "@Controller", "@Get", "render", "class_extends"
    pub trigger_key: String,

    /// 2. Pattern: Optional Regex to capture specific parts of the `value`.
    /// If None, the entire `value` is used.
    /// e.g., For value `"{ selector: 'app-root' }"`, Regex: `selector:\s*['"](.*?)['"]`
    pub extraction_regex: Option<Regex>,

    /// 3. Template: How to format the final string. Use `{}` for the captured value.
    /// e.g., "route:GET:{}" or "html:tag:{}"
    pub output_template: String,

    /// 4. Context Requirement: Does this rule depend on another hint existing?
    /// e.g., NestJS methods (@Get) depend on the class (@Controller) path.
    pub parent_context_key: Option<String>,
}

/// Defines a specific Framework (e.g., "Spring Boot", "Flask", "Angular")
pub struct FrameworkSpec {
    pub name: String,
    pub language: SupportedLanguage,

    /// How do we know this file uses this framework?
    /// e.g., file extension (".component.ts") OR generic import ("org.springframework")
    pub detection_import: Option<String>,
    pub detection_suffix: Option<String>,

    /// The rules to run
    pub rules: Vec<ConceptRule>,
}

pub struct FrameworkManager {
    specs: Vec<FrameworkSpec>,
}

impl FrameworkManager {
    pub fn new() -> Self {
        Self { specs: Vec::new() }
    }

    pub fn register(&mut self, spec: FrameworkSpec) {
        self.specs.push(spec);
    }
}

impl InferenceRule for FrameworkManager {
    fn infer_definitions(&self, boundary: &FileBoundary) -> Vec<String> {
        let mut results = Vec::new();

        // 1. Identify which frameworks apply to this file
        let active_specs: Vec<&FrameworkSpec> = self
            .specs
            .iter()
            .filter(|spec| {
                // Check File Extension
                /* (Logic to check boundary.path extension vs spec.language) */

                // Check Suffix (e.g. .component.ts)
                if let Some(suffix) = &spec.detection_suffix {
                    if !boundary.path.ends_with(suffix) {
                        return false;
                    }
                }

                // Check Imports (Did we import 'flask', 'spring', etc?)
                if let Some(imp) = &spec.detection_import {
                    // heuristic: check if any import contains the marker
                    if !boundary.imports.iter().any(|i| i.contains(imp)) {
                        return false;
                    }
                }

                true
            })
            .collect();

        if active_specs.is_empty() {
            return results;
        }

        // 2. Execute Rules
        for spec in active_specs {
            // Pre-calculation for "Context" (e.g. finding the Controller base path)
            let mut context_values: HashMap<String, String> = HashMap::new();

            // Pass 1: Gather Context (Rules that don't depend on parents)
            for rule in &spec.rules {
                if rule.parent_context_key.is_none() {
                    for hint in &boundary.framework_hints {
                        if hint.key == rule.trigger_key {
                            if let Some(val) = extract_value(hint, &rule.extraction_regex) {
                                context_values.insert(rule.trigger_key.clone(), val.clone());
                                // Also generate the independent definition
                                results.push(format_output(&rule.output_template, &val, None));
                            }
                        }
                    }
                }
            }

            // Pass 2: Dependent Rules (e.g. Methods that need Controller path)
            for rule in &spec.rules {
                if let Some(parent_key) = &rule.parent_context_key {
                    if let Some(parent_val) = context_values.get(parent_key) {
                        for hint in &boundary.framework_hints {
                            if hint.key == rule.trigger_key {
                                if let Some(val) = extract_value(hint, &rule.extraction_regex) {
                                    // Combine Parent + Child (e.g. /api + /users)
                                    // We pass the parent_val to the formatter
                                    results.push(format_output(
                                        &rule.output_template,
                                        &val,
                                        Some(parent_val),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        results
    }
}

// --- Helpers ---

fn extract_value(hint: &FrameworkHint, regex: &Option<Regex>) -> Option<String> {
    if let Some(re) = regex {
        re.captures(&hint.value)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    } else {
        Some(hint.value.clone())
    }
}

fn format_output(template: &str, value: &str, context: Option<&String>) -> String {
    let mut out = template.replace("{}", value);
    if let Some(ctx) = context {
        out = out.replace("{parent}", ctx);
    }
    // Cleanup double slashes in routes
    out.replace("//", "/")
}
