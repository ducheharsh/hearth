//! Container inventory and live resource usage via the Docker Engine API.

use std::collections::HashMap;

use bollard::Docker;
use bollard::query_parameters::{
    InspectContainerOptions, ListContainersOptionsBuilder, LogsOptions, RestartContainerOptions,
    StartContainerOptions, StatsOptionsBuilder, StopContainerOptions,
};
use futures_util::Stream;
use futures_util::{StreamExt, future::join_all};
use serde::Serialize;

/// Label docker compose puts on every container.
pub const COMPOSE_PROJECT_LABEL: &str = "com.docker.compose.project";

#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error("cannot connect to Docker (is OrbStack running?): {0}")]
    Connect(#[source] bollard::errors::Error),
    #[error("Docker API error: {0}")]
    Api(#[from] bollard::errors::Error),
}

/// One container with optional live usage.
#[derive(Debug, Clone, Serialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    /// Compose project, if the container belongs to one.
    pub project: Option<String>,
    pub mem_bytes: Option<u64>,
    pub cpu_percent: Option<f64>,
}

/// Thin, cloneable handle to the Docker engine.
#[derive(Debug, Clone)]
pub struct DockerClient {
    inner: Docker,
}

impl DockerClient {
    /// Connect using `DOCKER_HOST` or the default socket, falling back to `OrbStack`'s socket.
    pub async fn connect() -> Result<Self, DockerError> {
        let primary = Docker::connect_with_defaults().map_err(DockerError::Connect)?;
        if primary.ping().await.is_ok() {
            return Ok(Self { inner: primary });
        }
        let home = std::env::var("HOME").unwrap_or_default();
        let orb = format!("{home}/.orbstack/run/docker.sock");
        let fallback = Docker::connect_with_unix(&orb, 30, bollard::API_DEFAULT_VERSION)
            .map_err(DockerError::Connect)?;
        fallback.ping().await.map_err(DockerError::Connect)?;
        Ok(Self { inner: fallback })
    }

    /// All containers (running or not), with usage for running ones.
    pub async fn containers(&self) -> Result<Vec<ContainerInfo>, DockerError> {
        let opts = ListContainersOptionsBuilder::default().all(true).build();
        let list = self.inner.list_containers(Some(opts)).await?;

        let futures = list.into_iter().map(|c| async move {
            let id = c.id.unwrap_or_default();
            let state = c.state.map(|s| s.to_string()).unwrap_or_default();
            let usage = if state == "running" {
                self.usage(&id).await
            } else {
                None
            };
            let labels: HashMap<String, String> = c.labels.unwrap_or_default();
            ContainerInfo {
                name: c.names.and_then(|n| n.into_iter().next()).map_or_else(
                    || id.chars().take(12).collect(),
                    |n| n.trim_start_matches('/').to_owned(),
                ),
                id,
                image: c.image.unwrap_or_default(),
                state,
                status: c.status.unwrap_or_default(),
                project: labels.get(COMPOSE_PROJECT_LABEL).cloned(),
                mem_bytes: usage.map(|u| u.0),
                cpu_percent: usage.map(|u| u.1),
            }
        });
        let mut out = join_all(futures).await;
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// (memory bytes, cpu %) for a running container; `None` if unavailable.
    async fn usage(&self, id: &str) -> Option<(u64, f64)> {
        // stream=false, one_shot=false: daemon waits one cycle so precpu is populated.
        let opts = StatsOptionsBuilder::default()
            .stream(false)
            .one_shot(false)
            .build();
        let stats = self.inner.stats(id, Some(opts)).next().await?.ok()?;

        let mem = stats.memory_stats.as_ref().and_then(|m| {
            let usage = m.usage?;
            // Match `docker stats`: subtract reclaimable page cache.
            let cache = m
                .stats
                .as_ref()
                .and_then(|s| s.get("inactive_file").copied())
                .unwrap_or(0);
            Some(usage.saturating_sub(cache))
        })?;

        let cpu = (|| {
            let cur = stats.cpu_stats.as_ref()?;
            let pre = stats.precpu_stats.as_ref()?;
            let total = cur.cpu_usage.as_ref()?.total_usage?;
            let pre_total = pre.cpu_usage.as_ref()?.total_usage?;
            let sys = cur.system_cpu_usage?;
            let pre_sys = pre.system_cpu_usage?;
            let cpus = f64::from(cur.online_cpus.unwrap_or(1).max(1));
            Some(cpu_percent(total, pre_total, sys, pre_sys, cpus))
        })()
        .unwrap_or(0.0);

        Some((mem, cpu))
    }
}

/// Container lifecycle actions exposed in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Start,
    Stop,
    Restart,
}

/// Everything the container detail view shows.
#[derive(Debug, Clone, Serialize)]
pub struct ContainerDetail {
    pub id: String,
    pub name: String,
    pub image: String,
    pub created: Option<String>,
    pub status: String,
    pub running: bool,
    pub started_at: Option<String>,
    pub health: Option<String>,
    pub exit_code: Option<i64>,
    pub restart_count: Option<i64>,
    pub restart_policy: Option<String>,
    pub project: Option<String>,
    pub service: Option<String>,
    /// `"container/proto" -> ["host_ip:host_port", ...]`
    pub ports: Vec<PortBinding>,
    pub mounts: Vec<MountInfo>,
    pub networks: Vec<String>,
    /// Environment with secret-looking values redacted.
    pub env: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PortBinding {
    pub container: String,
    pub host: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MountInfo {
    pub kind: String,
    pub source: String,
    pub destination: String,
    pub read_only: bool,
}

/// One log line.
#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub stream: &'static str,
    pub text: String,
}

const SECRET_HINTS: &[&str] = &["PASS", "SECRET", "TOKEN", "KEY", "PRIVATE", "CREDENTIAL"];

/// Hide values of env vars whose names look secret.
#[must_use]
pub fn redact_env(kv: &str) -> String {
    match kv.split_once('=') {
        Some((k, _))
            if SECRET_HINTS
                .iter()
                .any(|h| k.to_ascii_uppercase().contains(h)) =>
        {
            format!("{k}=••••••")
        }
        _ => kv.to_owned(),
    }
}

impl DockerClient {
    pub async fn act(&self, id: &str, action: Action) -> Result<(), DockerError> {
        match action {
            Action::Start => {
                self.inner
                    .start_container(id, None::<StartContainerOptions>)
                    .await?;
            }
            Action::Stop => {
                self.inner
                    .stop_container(id, None::<StopContainerOptions>)
                    .await?;
            }
            Action::Restart => {
                self.inner
                    .restart_container(id, None::<RestartContainerOptions>)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn inspect(&self, id: &str) -> Result<ContainerDetail, DockerError> {
        let c = self
            .inner
            .inspect_container(id, None::<InspectContainerOptions>)
            .await?;
        let state = c.state.as_ref();
        let config = c.config.as_ref();
        let labels = config.and_then(|c| c.labels.clone()).unwrap_or_default();
        let mut ports: Vec<PortBinding> = c
            .network_settings
            .as_ref()
            .and_then(|n| n.ports.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|(container, binds)| PortBinding {
                container,
                host: binds
                    .unwrap_or_default()
                    .into_iter()
                    .map(|b| {
                        format!(
                            "{}:{}",
                            b.host_ip.unwrap_or_default(),
                            b.host_port.unwrap_or_default()
                        )
                    })
                    .collect(),
            })
            .collect();
        ports.sort_by(|a, b| a.container.cmp(&b.container));
        Ok(ContainerDetail {
            id: c.id.clone().unwrap_or_default(),
            name: c
                .name
                .clone()
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_owned(),
            image: config.and_then(|c| c.image.clone()).unwrap_or_default(),
            created: c.created.clone(),
            status: state
                .and_then(|s| s.status.map(|st| st.to_string()))
                .unwrap_or_default(),
            running: state.and_then(|s| s.running).unwrap_or(false),
            started_at: state.and_then(|s| s.started_at.clone()),
            health: state
                .and_then(|s| s.health.as_ref())
                .and_then(|h| h.status.map(|st| st.to_string())),
            exit_code: state.and_then(|s| s.exit_code),
            restart_count: c.restart_count,
            restart_policy: c
                .host_config
                .as_ref()
                .and_then(|h| h.restart_policy.as_ref())
                .and_then(|r| r.name.map(|n| n.to_string())),
            project: labels.get(COMPOSE_PROJECT_LABEL).cloned(),
            service: labels.get("com.docker.compose.service").cloned(),
            ports,
            mounts: c
                .mounts
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|m| MountInfo {
                    kind: m.typ.unwrap_or_default(),
                    source: m.source.unwrap_or_default(),
                    destination: m.destination.unwrap_or_default(),
                    read_only: !m.rw.unwrap_or(true),
                })
                .collect(),
            networks: c
                .network_settings
                .as_ref()
                .and_then(|n| n.networks.as_ref())
                .map(|n| n.keys().cloned().collect())
                .unwrap_or_default(),
            env: config
                .and_then(|c| c.env.clone())
                .unwrap_or_default()
                .iter()
                .map(|kv| redact_env(kv))
                .collect(),
        })
    }

    /// Recent log lines (`tail`), or a live stream when `follow` is set.
    pub fn logs(
        &self,
        id: &str,
        tail: usize,
        follow: bool,
    ) -> impl Stream<Item = Result<LogLine, DockerError>> + Send + use<> {
        let opts = LogsOptions {
            follow,
            stdout: true,
            stderr: true,
            timestamps: true,
            tail: tail.to_string(),
            ..LogsOptions::default()
        };
        self.inner.logs(id, Some(opts)).map(|r| {
            r.map(|out| {
                let stream = match &out {
                    bollard::container::LogOutput::StdErr { .. } => "stderr",
                    _ => "stdout",
                };
                LogLine {
                    stream,
                    text: out.to_string().trim_end_matches('\n').to_owned(),
                }
            })
            .map_err(DockerError::from)
        })
    }
}

/// An interactive shell inside a container.
pub struct Shell {
    pub exec_id: String,
    pub output: std::pin::Pin<
        Box<
            dyn Stream<Item = Result<bollard::container::LogOutput, bollard::errors::Error>> + Send,
        >,
    >,
    pub input: std::pin::Pin<Box<dyn tokio::io::AsyncWrite + Send>>,
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell")
            .field("exec_id", &self.exec_id)
            .finish_non_exhaustive()
    }
}

impl DockerClient {
    /// Start `bash` (or `sh` if bash is missing) with a TTY.
    pub async fn shell(&self, id: &str) -> Result<Shell, DockerError> {
        use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
        let exec = self
            .inner
            .create_exec(
                id,
                CreateExecOptions {
                    attach_stdin: Some(true),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    tty: Some(true),
                    env: Some(vec!["TERM=xterm-256color"]),
                    cmd: Some(vec![
                        "/bin/sh",
                        "-c",
                        "if command -v bash >/dev/null 2>&1; then exec bash; else exec sh; fi",
                    ]),
                    ..CreateExecOptions::default()
                },
            )
            .await?;
        match self
            .inner
            .start_exec(
                &exec.id,
                Some(StartExecOptions {
                    detach: false,
                    tty: true,
                    output_capacity: None,
                }),
            )
            .await?
        {
            StartExecResults::Attached { output, input } => Ok(Shell {
                exec_id: exec.id,
                output,
                input,
            }),
            StartExecResults::Detached => Err(DockerError::Api(
                bollard::errors::Error::DockerResponseServerError {
                    status_code: 500,
                    message: "exec started detached".into(),
                },
            )),
        }
    }

    pub async fn resize_shell(
        &self,
        exec_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<(), DockerError> {
        self.inner
            .resize_exec(
                exec_id,
                bollard::exec::ResizeExecOptions {
                    height: rows,
                    width: cols,
                },
            )
            .await?;
        Ok(())
    }
}

impl DockerClient {
    /// `repo@sha256:…` digests the local copy of `image` is known by.
    pub async fn local_digests(&self, image: &str) -> Result<Vec<String>, DockerError> {
        Ok(self
            .inner
            .inspect_image(image)
            .await?
            .repo_digests
            .unwrap_or_default())
    }

    /// Digest the registry currently serves for `image` (no download).
    pub async fn remote_digest(&self, image: &str) -> Result<Option<String>, DockerError> {
        let d = self.inner.inspect_registry_image(image, None).await?;
        Ok(d.descriptor.digest)
    }
}

/// Is `remote` (a bare `sha256:…`) among the local `repo@sha256:…` digests?
#[must_use]
pub fn has_digest(local: &[String], remote: &str) -> bool {
    local.iter().any(|d| d.rsplit('@').next() == Some(remote))
}

/// Same formula as the Docker CLI.
#[allow(clippy::cast_precision_loss)]
fn cpu_percent(total: u64, pre_total: u64, sys: u64, pre_sys: u64, cpus: f64) -> f64 {
    let cpu_delta = total.saturating_sub(pre_total) as f64;
    let sys_delta = sys.saturating_sub(pre_sys) as f64;
    if sys_delta > 0.0 {
        cpu_delta / sys_delta * cpus * 100.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_matching() {
        let local = vec!["louislam/uptime-kuma@sha256:abc".to_owned()];
        assert!(has_digest(&local, "sha256:abc"));
        assert!(!has_digest(&local, "sha256:def"));
        assert!(!has_digest(&[], "sha256:abc"));
    }

    #[test]
    fn redacts_secret_env() {
        assert_eq!(redact_env("DB_PASSWORD=hunter2"), "DB_PASSWORD=••••••");
        assert_eq!(redact_env("api_key=x"), "api_key=••••••");
        assert_eq!(redact_env("TZ=UTC"), "TZ=UTC");
        assert_eq!(redact_env("NOVALUE"), "NOVALUE");
    }

    #[test]
    fn cpu_formula_matches_docker_cli() {
        // 10% of system time across 4 CPUs => 40%
        let p = cpu_percent(200, 100, 2_000, 1_000, 4.0);
        assert!((p - 40.0).abs() < 1e-9);
        assert!((cpu_percent(5, 5, 10, 10, 2.0)).abs() < f64::EPSILON);
    }
}
