use super::InferenceRule;
use crate::models::FileBoundary;

/// A single, stateless rule that checks a file path and infers definitions.
trait ConventionRule: Send + Sync {
    fn infer_from_path(&self, path: &str) -> Vec<String>;
}

// --- Specific Convention Implementations ---

/// Matches Next.js Pages Router convention: `src/pages/api/user.ts` -> `route:/api/user`
struct NextJsPagesRouter;
impl ConventionRule for NextJsPagesRouter {
    fn infer_from_path(&self, path: &str) -> Vec<String> {
        if let Some(idx) = path.find("/pages/api/") {
            let relative = &path[idx + "/pages".len()..];
            if let Some(dot_idx) = relative.rfind('.') {
                let mut route = relative[..dot_idx].to_string();
                if route.ends_with("/index") {
                    route = route.trim_end_matches("/index").to_string();
                }
                // Handle empty route after trimming index (e.g., /pages/api/index.ts)
                if route.is_empty() {
                    route.push('/');
                }
                return vec![format!("route:{}", route)];
            }
        }
        vec![]
    }
}

/// Matches Next.js App Router convention: `app/api/auth/route.ts` -> `route:/api/auth`
struct NextJsAppRouter;
impl ConventionRule for NextJsAppRouter {
    fn infer_from_path(&self, path: &str) -> Vec<String> {
        if path.contains("/app/") && (path.ends_with("/route.ts") || path.ends_with("/route.js")) {
            if let Some(app_idx) = path.find("/app/") {
                if let Some(route_idx) = path.rfind("/route.") {
                    let relative = &path[app_idx + "/app".len()..route_idx];
                    return vec![format!("route:{}", relative)];
                }
            }
        }
        vec![]
    }
}

/// Placeholder for Ruby on Rails controllers.
/// e.g., `app/controllers/users_controller.rb` -> `controller:users`
struct RailsControllerConvention;
impl ConventionRule for RailsControllerConvention {
    fn infer_from_path(&self, path: &str) -> Vec<String> {
        if path.contains("app/controllers/") && path.ends_with("_controller.rb") {
            // Basic regex or string splitting can extract the controller name here.
        }
        vec![]
    }
}

// --- The Main Engine for this file ---

/// The primary struct that implements `InferenceRule` for path-based conventions.
/// It holds and executes a list of specific convention rules.
pub struct ConventionEngine {
    rules: Vec<Box<dyn ConventionRule>>,
}

impl ConventionEngine {
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(NextJsPagesRouter),
                Box::new(NextJsAppRouter),
                Box::new(RailsControllerConvention),
                // Add more convention rules here (e.g., for SvelteKit, Nuxt.js)
            ],
        }
    }
}

impl InferenceRule for ConventionEngine {
    fn infer_definitions(&self, boundary: &FileBoundary) -> Vec<String> {
        let mut defs = Vec::new();
        // Always normalize path separators for consistent matching
        let normalized_path = boundary.path.replace('\\', "/");

        for rule in &self.rules {
            defs.extend(rule.infer_from_path(&normalized_path));
        }

        defs
    }
}
