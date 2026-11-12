use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepackageSecurityError {
    #[error("Detected dangerous command execution pattern: {pattern} at line {line}")]
    CommandInjectionTell { pattern: String, line: usize },
    #[error("Detected arbitrary code evaluation (eval / Function constructor) at line {line}")]
    ArbitraryCodeEvaluation { line: usize },
    #[error("Detected suspicious network socket or exfiltration call: {pattern} at line {line}")]
    UnauthorizedNetworkCall { pattern: String, line: usize },
    #[error("Detected path traversal or unsafe file access: {pattern} at line {line}")]
    PathTraversal { pattern: String, line: usize },
}

/// Zero-Trust security scanner for external skills and imported repositories (AC-03)
pub fn scan_external_script_security(
    filename: &str,
    content: &str,
) -> Result<(), RepackageSecurityError> {
    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let lower = line.to_lowercase();

        // 1. Detect command injection and shell spawning
        let shell_tells = [
            "curl | bash",
            "curl | sh",
            "wget -o- | bash",
            "child_process.exec",
            "child_process.spawn",
            "system(",
            "popen(",
            "subprocess.call",
            "subprocess.popen",
            "rm -rf /",
        ];
        for tell in &shell_tells {
            if lower.contains(tell) {
                return Err(RepackageSecurityError::CommandInjectionTell {
                    pattern: tell.to_string(),
                    line: line_num,
                });
            }
        }

        // 2. Detect arbitrary code execution
        if lower.contains("eval(")
            || lower.contains("new function(")
            || lower.contains("window.eval")
            || lower.contains("exec(")
                && (filename.ends_with(".py")
                    || filename.ends_with(".js")
                    || filename.ends_with(".ts"))
        {
            return Err(RepackageSecurityError::ArbitraryCodeEvaluation { line: line_num });
        }

        // 3. Detect unauthorized network calls in sandboxed scripts
        let network_tells = [
            "nc -e",
            "/bin/sh",
            "/bin/bash",
            "socket.connect",
            "telemetry-exfil.com",
        ];
        for tell in &network_tells {
            if lower.contains(tell) {
                return Err(RepackageSecurityError::UnauthorizedNetworkCall {
                    pattern: tell.to_string(),
                    line: line_num,
                });
            }
        }

        // 4. Detect path traversal
        if lower.contains("../../../") || lower.contains("/etc/passwd") || lower.contains("/root/")
        {
            return Err(RepackageSecurityError::PathTraversal {
                pattern: line.trim().to_string(),
                line: line_num,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passes_clean_script() {
        let clean = r#"
export function calculatePadding(w: number): number {
    return Math.max(16, w * 0.1);
}
"#;
        assert!(scan_external_script_security("utils.ts", clean).is_ok());
    }

    #[test]
    fn test_rejects_curl_bash() {
        let malicious = "run_setup() {\n  curl | bash\n}";
        let err = scan_external_script_security("install.sh", malicious).unwrap_err();
        match err {
            RepackageSecurityError::CommandInjectionTell { pattern, line } => {
                assert_eq!(pattern, "curl | bash");
                assert_eq!(line, 2);
            }
            _ => panic!("Expected CommandInjectionTell"),
        }
    }

    #[test]
    fn test_rejects_eval() {
        let malicious = "const res = eval(userInput);";
        let err = scan_external_script_security("calc.js", malicious).unwrap_err();
        assert_eq!(
            err,
            RepackageSecurityError::ArbitraryCodeEvaluation { line: 1 }
        );
    }
}
