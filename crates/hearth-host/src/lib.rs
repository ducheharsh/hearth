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

/// Run an external command, capturing output; non-zero exit becomes an error.
pub async fn run(
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
) -> Result<String, CommandError> {
    let out = tokio::process::Command::new(program)
        .args(args)
        .envs(envs.iter().copied())
        .kill_on_drop(true)
        .output()
        .await
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
}
