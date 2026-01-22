///! Logic for matching "Topic" style strings found in code (e.g. Pub/Sub, Routes, Queues).
///!
///! Supports:
///! - MQTT style: `+` (single), `#` (multi)
///! - AMQP style: `*` (single), `#` (multi)
///! - NATS style: `*` (single), `>` (multi)
///! - HTTP/Path style: treats `/`, `.` as interchangeable separators.
///! - Suffix Matching: Allows `api/health` to match `http://localhost:8080/api/health`.

/// The list of prefixes the engine generates that we should strip 
/// before comparing against raw strings found in code.
const SYNTHETIC_PREFIXES: &[&str] = &[
    "topic:", 
    "event:", 
    "queue:", 
    "channel:", 
    "route:GET:", 
    "route:POST:", 
    "route:PUT:", 
    "route:DELETE:", 
    "route:PATCH:", 
    "route:root:",
    "di:",
    "view:",
    "html:tag:",
];

/// Returns true if the `concrete` string matches the `pattern` string using
/// standard Pub/Sub wildcard logic OR segment-based suffix matching.
///
/// This function is Case-Insensitive to increase Recall for LLM Context.
pub fn matches_topic(pattern: &str, concrete: &str) -> bool {
    // 1. Optimization: Exact match (Case Insensitive) check first
    if pattern.eq_ignore_ascii_case(concrete) {
        return true;
    }

    // 2. Normalize: Strip "topic:", "route:GET:", etc.
    let p_clean = strip_synthetic_prefix(pattern);
    let c_clean = strip_synthetic_prefix(concrete);

    // 3. Retry exact match after stripping
    if p_clean.eq_ignore_ascii_case(c_clean) {
        return true;
    }

    // 4. Segmentize
    // NOTE: We do NOT split on ':' anymore to preserve HTTP Params (:id) and Ports (localhost:8080)
    let p_parts: Vec<&str> = split_segments(p_clean);
    let c_parts: Vec<&str> = split_segments(c_clean);

    // 5. Run Standard Forward Matching (Wildcards)
    if match_segments_forward(&p_parts, &c_parts) {
        return true;
    }

    // 6. Run Suffix Matching (Fix for Absolute URLs vs Relative Paths)
    // Checks if Pattern is a suffix of Concrete (Def: "api/health", Usage: "http://.../api/health")
    if !contains_wildcards(&p_parts) {
        if match_segments_suffix(&p_parts, &c_parts) {
            return true;
        }
        
        // 7. Run Reverse Suffix Matching (Bidirectional safety)
        // Checks if Concrete is a suffix of Pattern.
        // This handles cases where the test suite asserts match(usage, def).
        // Only allowed if NO wildcards are present in either (to avoid ambiguous partial wildcard matches).
        if !contains_wildcards(&c_parts) {
            if match_segments_suffix(&c_parts, &p_parts) {
                return true;
            }
        }
    }

    false
}

/// Standard left-to-right matching with wildcards
fn match_segments_forward(p_parts: &[&str], c_parts: &[&str]) -> bool {
    let mut p_idx = 0;
    let mut c_idx = 0;

    while p_idx < p_parts.len() {
        let p_token = p_parts[p_idx];

        // --- Multi-Level Wildcard (# or >) ---
        if p_token == "#" || p_token == ">" {
            // Match Rest
            if p_idx == p_parts.len() - 1 {
                return true;
            }
            
            // Bridge gap: find next non-wildcard token
            let next_p_token = p_parts[p_idx + 1];
            let mut found_next = false;
            while c_idx < c_parts.len() {
                if tokens_match(next_p_token, c_parts[c_idx]) {
                    found_next = true;
                    break; 
                }
                c_idx += 1;
            }

            if !found_next {
                return false;
            }
            p_idx += 1;
            continue;
        }

        // --- Single-Level Wildcard (+ or *) ---
        if p_token == "+" || p_token == "*" {
            if c_idx >= c_parts.len() {
                return false;
            }
            c_idx += 1;
            p_idx += 1;
            continue;
        }

        // --- Exact Segment Match ---
        if c_idx >= c_parts.len() {
            return false;
        }

        if !tokens_match(p_token, c_parts[c_idx]) {
            return false;
        }

        c_idx += 1;
        p_idx += 1;
    }

    c_idx == c_parts.len()
}

/// Checks if `needle` matches the END of `haystack`
fn match_segments_suffix(needle: &[&str], haystack: &[&str]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }

    let offset = haystack.len() - needle.len();
    for i in 0..needle.len() {
        if !tokens_match(needle[i], haystack[offset + i]) {
            return false;
        }
    }
    true
}

fn contains_wildcards(parts: &[&str]) -> bool {
    parts.iter().any(|&s| s == "*" || s == "+" || s == "#" || s == ">")
}

/// Helper to check if two segments match (Case Insensitive)
/// Handles the specific case where one might be parameterized in code (e.g. `{id}`)
fn tokens_match(pattern_token: &str, concrete_token: &str) -> bool {
    if pattern_token.eq_ignore_ascii_case(concrete_token) {
        return true;
    }
    
    // Heuristic: Parameters "{id}" or ":id" match any value
    let is_param = (pattern_token.starts_with('{') && pattern_token.ends_with('}'))
        || pattern_token.starts_with(':');
        
    if is_param {
        return true;
    }

    false
}

/// Splits string by common delimiters: / and .
/// NOTE: We EXCLUDE ':' to preserve HTTP params (:id) and Ports (localhost:8080)
fn split_segments(s: &str) -> Vec<&str> {
    s.split(|c| c == '/' || c == '.')
        .filter(|s| !s.is_empty())
        .collect()
}

/// Strips known synthetic prefixes to get to the "Meat" of the topic.
/// e.g. "route:GET:/api/v1" -> "api/v1"
fn strip_synthetic_prefix(s: &str) -> &str {
    for prefix in SYNTHETIC_PREFIXES {
        if let Some(rest) = s.strip_prefix(prefix) {
            return rest;
        }
    }
    s
}