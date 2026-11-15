//! Native Ollama (runs on the Mac so models get the Metal GPU; Docker on macOS can't).
//!
//! Installed with Homebrew and kept running by `brew services` (launchd).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::CommandError;

const API: &str = "http://127.0.0.1:11434";
/// Where Docker containers reach the native server.
pub const FROM_CONTAINERS: &str = "http://host.docker.internal:11434";

#[derive(Debug, thiserror::Error)]
pub enum OllamaError {
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error("Ollama API: {0}")]
    Http(#[from] reqwest::Error),
    #[error(
        "Homebrew is needed to install Ollama. Install it from brew.sh or get Ollama from ollama.com."
    )]
    NoBrew,
    #[error("Ollama didn't start within a minute")]
    NotStarting,
    #[error("pulling {model}: {message}")]
    Pull { model: String, message: String },
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct OllamaStatus {
    /// The `ollama` binary exists.
    pub installed: bool,
    /// The API answers.
    pub running: bool,
    pub version: Option<String>,
    /// Models on disk, e.g. `llama3.2:3b`.
    pub models: Vec<String>,
}

fn client(timeout: Duration) -> Result<reqwest::Client, OllamaError> {
    Ok(reqwest::Client::builder().timeout(timeout).build()?)
}

fn which(bin: &str) -> Option<std::path::PathBuf> {
    [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/Applications/Ollama.app/Contents/Resources",
    ]
    .iter()
    .map(|d| std::path::Path::new(d).join(bin))
    .find(|p| p.exists())
}

pub async fn status() -> OllamaStatus {
    #[derive(Deserialize)]
    struct Version {
        version: String,
    }
    #[derive(Deserialize)]
    struct Tags {
        models: Vec<Tag>,
    }
    #[derive(Deserialize)]
    struct Tag {
        name: String,
    }
    let installed = which("ollama").is_some();
    let Ok(c) = client(Duration::from_secs(3)) else {
        return OllamaStatus {
            installed,
            ..OllamaStatus::default()
        };
    };
    let version = match c.get(format!("{API}/api/version")).send().await {
        Ok(r) => r.json::<Version>().await.ok().map(|v| v.version),
        Err(_) => None,
    };
    let models = match c.get(format!("{API}/api/tags")).send().await {
        Ok(r) => r
            .json::<Tags>()
            .await
            .map(|t| t.models.into_iter().map(|m| m.name).collect())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    OllamaStatus {
        installed: installed || version.is_some(),
        running: version.is_some(),
        version,
        models,
    }
}

/// Install with Homebrew (if needed) and start it as a login service.
pub async fn install_and_start() -> Result<(), OllamaError> {
    let brew = which("brew").ok_or(OllamaError::NoBrew)?;
    let brew = brew.to_string_lossy().into_owned();
    let env = [
        ("HOMEBREW_NO_AUTO_UPDATE", "1"),
        ("HOMEBREW_NO_INSTALL_CLEANUP", "1"),
    ];
    if which("ollama").is_none() {
        crate::run(&brew, &["install", "ollama"], &env).await?;
    }
    if !status().await.running {
        crate::run(&brew, &["services", "start", "ollama"], &env).await?;
    }
    for _ in 0..30 {
        if status().await.running {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err(OllamaError::NotStarting)
}

/// Download a model, reporting progress as a 0–100 percentage.
pub async fn pull(model: &str, mut progress: impl FnMut(u8) + Send) -> Result<(), OllamaError> {
    #[derive(Deserialize)]
    struct Line {
        #[serde(default)]
        status: String,
        total: Option<u64>,
        completed: Option<u64>,
        error: Option<String>,
    }
    let c = client(Duration::from_secs(6 * 3600))?;
    let mut res = c
        .post(format!("{API}/api/pull"))
        .json(&serde_json::json!({ "model": model, "stream": true }))
        .send()
        .await?
        .error_for_status()?;
    let mut buf = Vec::new();
    let mut last = 0u8;
    while let Some(chunk) = res.chunk().await? {
        buf.extend_from_slice(&chunk);
        while let Some(i) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=i).collect();
            let Ok(l) = serde_json::from_slice::<Line>(&line) else {
                continue;
            };
            if let Some(message) = l.error {
                return Err(OllamaError::Pull {
                    model: model.to_owned(),
                    message,
                });
            }
            if let (Some(t), Some(d)) = (l.total, l.completed)
                && t > 0
            {
                let pct = u8::try_from((d.saturating_mul(100) / t).min(100)).unwrap_or(100);
                if pct != last {
                    last = pct;
                    progress(pct);
                }
            }
            if l.status == "success" {
                progress(100);
                return Ok(());
            }
        }
    }
    Ok(())
}
