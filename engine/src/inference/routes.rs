use crate::models::FileBoundary;
use super::InferenceRule;

pub struct NextJsRouteRule;

impl InferenceRule for NextJsRouteRule {
    fn infer_definitions(&self, boundary: &FileBoundary) -> Vec<String> {
        let path = boundary.path.replace('\\', "/");
        let mut defs = Vec::new();

        // 1. Pages Router: src/pages/api/user.ts -> route:/api/user
        if let Some(idx) = path.find("/pages/api/") {
            let relative = &path[idx + "/pages".len()..]; 
            // Strip extension
            if let Some(dot_idx) = relative.rfind('.') {
                let mut route = relative[..dot_idx].to_string();
                if route.ends_with("/index") {
                    route = route[..route.len() - "/index".len()].to_string();
                }
                // Universal Link Key
                defs.push(format!("route:{}", route));
            }
        }

        // 2. App Router: app/api/auth/route.ts -> route:/api/auth
        // Logic: Must be inside /app/, must end in /route.{ts,js}
        if path.ends_with("/route.ts") || path.ends_with("/route.js") {
            if let Some(app_idx) = path.find("/app/") {
                if let Some(route_idx) = path.rfind("/route.") {
                    let relative = &path[app_idx + 4..route_idx]; 
                    defs.push(format!("route:{}", relative));
                }
            }
        }

        defs
    }
}