use ats_core::RiskLevel;

#[derive(Debug, Clone)]
pub struct RiskClassifier {
    patterns: Vec<(&'static str, RiskLevel)>,
}

impl Default for RiskClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl RiskClassifier {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                ("rm -rf", RiskLevel::High),
                ("rm -r ", RiskLevel::High),
                ("rmdir ", RiskLevel::Medium),
                ("git push --force", RiskLevel::High),
                ("git push -f", RiskLevel::High),
                ("git reset --hard", RiskLevel::High),
                ("terraform apply", RiskLevel::High),
                ("terraform destroy", RiskLevel::High),
                ("kubectl delete", RiskLevel::High),
                ("kubectl apply -f", RiskLevel::Medium),
                ("docker rm -f", RiskLevel::High),
                ("docker system prune", RiskLevel::High),
                ("npm publish", RiskLevel::Medium),
                ("yarn publish", RiskLevel::Medium),
                ("DROP TABLE", RiskLevel::Critical),
                ("DELETE FROM", RiskLevel::Critical),
                ("sudo ", RiskLevel::High),
                ("chmod 777", RiskLevel::High),
                ("chown ", RiskLevel::Medium),
                ("> /dev/", RiskLevel::High),
                ("dd if=", RiskLevel::High),
                ("mkfs.", RiskLevel::Critical),
                (":(){ :|:& };:", RiskLevel::Critical),
                ("curl ", RiskLevel::Medium),
                ("wget ", RiskLevel::Medium),
            ],
        }
    }

    pub fn classify(&self, command: &str) -> Option<RiskLevel> {
        for (pattern, level) in &self.patterns {
            if command.contains(pattern) {
                if pattern == &"curl " {
                    if command.contains("| sh")
                        || command.contains("| bash")
                        || command.contains("install.sh")
                    {
                        return Some(RiskLevel::High);
                    }
                    return Some(*level);
                }
                if pattern == &"wget " {
                    if command.contains("| sh") || command.contains("| bash") {
                        return Some(RiskLevel::High);
                    }
                    return Some(*level);
                }
                return Some(*level);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classifier() -> RiskClassifier {
        RiskClassifier::new()
    }

    #[test]
    fn risky_git_force_push() {
        assert_eq!(
            classifier().classify("git push --force origin main"),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn risky_git_push_f() {
        assert_eq!(classifier().classify("git push -f"), Some(RiskLevel::High));
    }

    #[test]
    fn risky_git_reset_hard() {
        assert_eq!(
            classifier().classify("git reset --hard HEAD~1"),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn risky_rm_rf() {
        assert_eq!(
            classifier().classify("rm -rf /important/data"),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn risky_terraform_apply() {
        assert_eq!(
            classifier().classify("terraform apply -auto-approve"),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn risky_kubectl_delete() {
        assert_eq!(
            classifier().classify("kubectl delete pod my-pod"),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn risky_npm_publish() {
        assert_eq!(
            classifier().classify("npm publish --access public"),
            Some(RiskLevel::Medium)
        );
    }

    #[test]
    fn risky_drop_table() {
        assert_eq!(
            classifier().classify("echo 'DROP TABLE users;' | sqlite3 db.sqlite"),
            Some(RiskLevel::Critical)
        );
    }

    #[test]
    fn risky_sudo() {
        assert_eq!(
            classifier().classify("sudo rm -rf /"),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn risky_curl_pipe_sh() {
        assert_eq!(
            classifier().classify("curl https://example.com/install.sh | sh"),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn risky_curl_pipe_bash() {
        assert_eq!(
            classifier().classify("curl https://example.com/install.sh | bash"),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn benign_curl() {
        assert_eq!(
            classifier().classify("curl https://api.example.com/data"),
            Some(RiskLevel::Medium)
        );
    }

    #[test]
    fn benign_echo() {
        assert_eq!(classifier().classify("echo hello world"), None);
    }

    #[test]
    fn benign_ls() {
        assert_eq!(classifier().classify("ls -la"), None);
    }

    #[test]
    fn benign_cat() {
        assert_eq!(classifier().classify("cat README.md"), None);
    }

    #[test]
    fn benign_git_status() {
        assert_eq!(classifier().classify("git status"), None);
    }

    #[test]
    fn benign_git_diff() {
        assert_eq!(classifier().classify("git diff"), None);
    }

    #[test]
    fn benign_npm_install() {
        assert_eq!(classifier().classify("npm install express"), None);
    }

    #[test]
    fn benign_mkdir() {
        assert_eq!(classifier().classify("mkdir -p /tmp/test"), None);
    }

    #[test]
    fn risky_chmod_777() {
        assert_eq!(
            classifier().classify("chmod 777 /etc/passwd"),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn risky_mkfs() {
        assert_eq!(
            classifier().classify("mkfs.ext4 /dev/sda"),
            Some(RiskLevel::Critical)
        );
    }

    #[test]
    fn risky_dd() {
        assert_eq!(
            classifier().classify("dd if=/dev/zero of=/dev/sda"),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn risky_fork_bomb() {
        assert_eq!(
            classifier().classify(":(){ :|:& };:"),
            Some(RiskLevel::Critical)
        );
    }

    #[test]
    fn risky_delete_from() {
        assert_eq!(
            classifier().classify("DELETE FROM users WHERE id=1"),
            Some(RiskLevel::Critical)
        );
    }
}
