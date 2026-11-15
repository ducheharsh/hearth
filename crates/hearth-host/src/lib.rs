//! Side-effecting adapters to the machine hearth runs on.
//!
//! hearth runs **natively** on the host (not in a container) so that these
//! adapters see the real Mac: real RAM/CPU/disks, native apps such as Ollama,
//! and the host's `tailscale` CLI.
//!
//! * [`system`]: CPU / memory / disk sampling via `sysinfo`.
//! * [`docker`]: container inventory and live stats via the Docker API.
//! * [`compose`]: installing/removing catalog apps with `docker compose`.
//! * [`tailscale`]: tailnet identity and `tailscale serve` publishing.

pub mod compose;
pub mod docker;
pub mod netbird;
pub mod ollama;
pub mod system;
pub mod tailscale;
pub mod volumes;

/// `n` random bytes from the OS, hex-encoded.
pub fn random_hex(n: usize) -> std::io::Result<String> {
    use std::fmt::Write as _;
    use std::io::Read as _;
    let mut bytes = vec![0u8; n];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().fold(String::with_capacity(n * 2), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    }))
}

/// `n` random bytes from the OS.
pub fn random_bytes<const N: usize>() -> std::io::Result<[u8; N]> {
    use std::io::Read as _;
    let mut bytes = [0u8; N];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// Find a command-line tool even when `PATH` is minimal (launchd services, Homebrew
/// services): the user's `PATH` first, then the usual install locations for Homebrew,
/// OrbStack, Docker Desktop and the Tailscale app. Falls back to the bare name.
#[must_use]
pub fn resolve(program: &str) -> std::path::PathBuf {
    use std::path::{Path, PathBuf};
    if program.contains('/') {
        return PathBuf::from(program);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.extend(
        [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/Applications/Docker.app/Contents/Resources/bin",
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin",
        ]
        .iter()
        .map(PathBuf::from),
    );
    dirs.push(home.join(".orbstack/bin"));
    if let Some(found) = dirs.iter().map(|d| d.join(program)).find(|p| p.is_file()) {
        return found;
    }
    // The Mac App Store / standalone Tailscale ships its CLI inside the app.
    let app_cli = Path::new("/Applications/Tailscale.app/Contents/MacOS/Tailscale");
    if program == "tailscale" && app_cli.is_file() {
        return app_cli.to_path_buf();
    }
    PathBuf::from(program)
}

/// Run an external command, capturing output; non-zero exit becomes an error.
pub async fn run(
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
) -> Result<String, CommandError> {
    run_inner(program, args, envs, None).await
}

/// [`run`], but give up (and kill the process) after `limit`. For commands that may
/// stop and wait for a person, such as `tailscale serve` asking to enable HTTPS.
pub async fn run_timeout(
    program: &str,
    args: &[&str],
    limit: std::time::Duration,
) -> Result<String, CommandError> {
    run_inner(program, args, &[], Some(limit)).await
}

async fn run_inner(
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    limit: Option<std::time::Duration>,
) -> Result<String, CommandError> {
    let output = tokio::process::Command::new(resolve(program))
        .args(args)
        .envs(envs.iter().copied())
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = match limit {
        // Dropping the timed-out future drops the child, which kills it.
        Some(d) => tokio::time::timeout(d, output)
            .await
            .map_err(|_| CommandError::Timeout {
                command: format!("{program} {}", args.join(" ")),
                secs: d.as_secs(),
            })?,
        None => output.await,
    }
    .map_err(|source| CommandError::Spawn {
        program: program.to_owned(),
        source,
    })?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(CommandError::Failed {
            command: format!("{program} {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("could not start `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("`{command}` failed: {stderr}")]
    Failed { command: String, stderr: String },
    #[error("`{command}` didn't finish within {secs}s")]
    Timeout { command: String, secs: u64 },
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use std::path::PathBuf;

    #[test]
    fn resolves_tools_outside_path() {
        assert!(resolve("sh").is_absolute());
        assert_eq!(resolve("./x"), PathBuf::from("./x"));
        assert_eq!(
            resolve("definitely-not-a-tool"),
            PathBuf::from("definitely-not-a-tool")
        );
    }
}
