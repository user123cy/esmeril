use std::path::Path;
use std::process::Command;

pub struct Outcome {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run(bin: &str, args: &[&str], cwd: &Path) -> anyhow::Result<Outcome> {
    let output = Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run '{bin}': {e}"))?;
    Ok(Outcome {
        code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub fn summary(out: &Outcome) -> String {
    let mut lines = out
        .stdout
        .lines()
        .chain(out.stderr.lines())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .rev()
        .take(2)
        .collect::<Vec<_>>();
    lines.reverse();
    lines.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_takes_last_lines() {
        let out = Outcome {
            code: 0,
            stdout: "a\nb\nc\n".into(),
            stderr: String::new(),
        };
        assert_eq!(summary(&out), "b · c");
    }

    #[test]
    fn summary_skips_blank_lines() {
        let out = Outcome {
            code: 1,
            stdout: "a\n\n\n".into(),
            stderr: "boom\n".into(),
        };
        assert_eq!(summary(&out), "a · boom");
    }
}
