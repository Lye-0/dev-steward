pub fn output(text: &str) -> String {
    let url = regex::Regex::new(r"(https?://)[^\s/@]+:[^\s/@]+@").expect("static regex");
    let secret = regex::Regex::new(
        r"(?i)((?:token|password|secret|api[_-]?key|authorization)\s*[:=]\s*)[^\s,;]+",
    )
    .expect("static regex");
    let github = regex::Regex::new(r"(?:gh[pousr]_[A-Za-z0-9_]{20,}|github_pat_[A-Za-z0-9_]{20,})")
        .expect("static regex");
    let text = url.replace_all(text, "$1[REDACTED]@");
    let text = secret.replace_all(&text, "$1[REDACTED]");
    github.replace_all(&text, "[REDACTED]").to_string()
}
#[cfg(test)]
mod tests {
    #[test]
    fn redacts_common_credentials() {
        let s = super::output(
            "https://me:password@example.org/x TOKEN=private-value ghp_123456789012345678901234567890",
        );
        assert!(!s.contains("password"));
        assert!(!s.contains("private-value"));
        assert!(!s.contains("123456789"));
    }
}
