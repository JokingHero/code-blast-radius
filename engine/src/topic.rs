/// Returns true if `concrete` matches `pattern` using standard Pub/Sub wildcards.
/// Supports:
/// - `*` (Single level wildcard)
/// - `#` or `>` (Multi-level wildcard/recursive)
/// - Delimiters: `.`, `/`, `:`
pub fn matches_topic(pattern: &str, concrete: &str) -> bool {
    // Optimization: exact match check first
    if pattern == concrete {
        return true;
    }

    // Determine delimiter (heuristic based on what is present)
    let delimiter = if pattern.contains('/') {
        '/'
    } else if pattern.contains(':') {
        ':'
    } else {
        '.'
    };

    let p_parts: Vec<&str> = pattern.split(delimiter).collect();
    let c_parts: Vec<&str> = concrete.split(delimiter).collect();

    let mut p_idx = 0;
    let mut c_idx = 0;

    while p_idx < p_parts.len() {
        let p_token = p_parts[p_idx];

        if p_token == "#" || p_token == ">" {
            // Multi-level wildcard must be the last token in standard MQTT,
            // but RabbitMQ allows it elsewhere. Let's assume it matches "the rest".
            return true;
        }

        if p_token == "*" {
            // Single level wildcard
            c_idx += 1;
            p_idx += 1;
            continue;
        }

        // Exact match required for this segment
        if c_idx >= c_parts.len() || p_token != c_parts[c_idx] {
            return false;
        }

        c_idx += 1;
        p_idx += 1;
    }

    // Ensure we consumed all of concrete (unless we hit a # earlier)
    c_idx == c_parts.len()
}
