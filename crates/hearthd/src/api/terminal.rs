//! Interactive container terminal over WebSocket.
//!
//! Protocol: binary frames carry raw TTY bytes both ways; text frames from the client are
//! JSON control messages: `{"type":"resize","cols":120,"rows":32}`.

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::get;
use futures_util::{SinkExt as _, StreamExt as _};
use serde::Deserialize;
use tokio::io::AsyncWriteExt as _;

use super::bad;
use crate::error::ApiError;
use crate::events::Level;
use crate::state::SharedState;

pub fn routes() -> Router<SharedState> {
    Router::new().route("/containers/{id}/terminal", get(terminal))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Control {
    Resize { cols: u16, rows: u16 },
}

async fn terminal(
    State(st): State<SharedState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(bad("Invalid container id."));
    }
    let docker = st
        .docker
        .clone()
        .ok_or_else(|| bad("Docker isn't reachable."))?;
    st.event(Level::Info, "containers", "Terminal opened", id.clone())
        .await;
    Ok(ws.on_upgrade(move |socket| async move {
        if let Err(e) = session(socket, docker, &id).await {
            tracing::warn!(container = %id, error = %e, "terminal session ended with error");
        }
    }))
}

async fn session(
    socket: WebSocket,
    docker: hearth_host::docker::DockerClient,
    id: &str,
) -> anyhow::Result<()> {
    let (mut tx, mut rx) = socket.split();
    let shell = match docker.shell(id).await {
        Ok(s) => s,
        Err(e) => {
            let _ = tx
                .send(Message::Text(
                    format!("\r\n\x1b[31mCan't start a shell: {e}\x1b[0m\r\n").into(),
                ))
                .await;
            return Ok(());
        }
    };
    let hearth_host::docker::Shell {
        exec_id,
        mut output,
        mut input,
    } = shell;

    let to_client = async {
        while let Some(chunk) = output.next().await {
            let bytes = chunk?.into_bytes();
            if tx.send(Message::Binary(bytes)).await.is_err() {
                break;
            }
        }
        let _ = tx.send(Message::Close(None)).await;
        anyhow::Ok(())
    };
    let from_client = async {
        while let Some(msg) = rx.next().await {
            match msg? {
                Message::Binary(b) => input.write_all(&b).await?,
                Message::Text(t) => match serde_json::from_str::<Control>(&t) {
                    Ok(Control::Resize { cols, rows }) if cols > 0 && rows > 0 => {
                        let _ = docker.resize_shell(&exec_id, cols, rows).await;
                    }
                    _ => input.write_all(t.as_bytes()).await?,
                },
                Message::Close(_) => break,
                _ => {}
            }
        }
        anyhow::Ok(())
    };
    tokio::select! {
        r = to_client => r?,
        r = from_client => r?,
    }
    Ok(())
}
