//! `hearthd service install|uninstall|status`: run hearth at login with launchd.
//!
//! Writes `~/Library/LaunchAgents/dev.hearth.hearthd.plist` (keep-alive, logs in
//! `<state_dir>/logs/`). Homebrew users get the same through `brew services start hearth`;
//! use one or the other.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, bail};

const LABEL: &str = "dev.hearth.hearthd";

fn home() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

fn plist_path() -> anyhow::Result<PathBuf> {
    Ok(home()?
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

fn domain() -> anyhow::Result<String> {
    let out = Command::new("/usr/bin/id")
        .arg("-u")
        .output()
        .context("running id -u")?;
    Ok(format!(
        "gui/{}",
        String::from_utf8_lossy(&out.stdout).trim()
    ))
}

fn launchctl(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("/bin/launchctl").args(args).output()
}

/// The launchd job definition (pure, so it can be tested).
#[must_use]
pub fn render_plist(exe: &Path, config: Option<&Path>, log: &Path) -> plist::Value {
    let mut args = vec![plist::Value::String(exe.display().to_string())];
    if let Some(c) = config {
        args.push("--config".into());
        args.push(plist::Value::String(c.display().to_string()));
    }
    let mut env = plist::Dictionary::new();
    env.insert("RUST_LOG".into(), "hearthd=info,tower_http=warn".into());
    let mut d = plist::Dictionary::new();
    d.insert("Label".into(), LABEL.into());
    d.insert("ProgramArguments".into(), plist::Value::Array(args));
    d.insert("RunAtLoad".into(), true.into());
    d.insert("KeepAlive".into(), true.into());
    d.insert("ProcessType".into(), "Interactive".into());
    d.insert("StandardOutPath".into(), log.display().to_string().into());
    d.insert("StandardErrorPath".into(), log.display().to_string().into());
    d.insert("EnvironmentVariables".into(), plist::Value::Dictionary(env));
    plist::Value::Dictionary(d)
}

pub fn install(config: Option<&Path>, cfg: &crate::config::Config) -> anyhow::Result<()> {
    if home()?
        .join("Library/LaunchAgents/homebrew.mxcl.hearth.plist")
        .exists()
    {
        bail!(
            "hearth already runs as a Homebrew service. Use `brew services restart hearth`, or `brew services stop hearth` first."
        );
    }
    let exe = std::env::current_exe()?.canonicalize()?;
    let config = config.map(Path::canonicalize).transpose()?;
    let logs = cfg.state_dir.join("logs");
    std::fs::create_dir_all(&logs)?;
    let path = plist_path()?;
    std::fs::create_dir_all(path.parent().context("no LaunchAgents dir")?)?;
    render_plist(&exe, config.as_deref(), &logs.join("hearthd.log")).to_file_xml(&path)?;
    let d = domain()?;
    let _ = launchctl(&["bootout", &format!("{d}/{LABEL}")]);
    let out = launchctl(&["bootstrap", &d, &path.display().to_string()])?;
    if !out.status.success() {
        bail!(
            "launchctl bootstrap failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    println!("hearth is running and will start at login.");
    println!("  Dashboard: http://{}", cfg.listen);
    println!("  Logs:      {}", logs.join("hearthd.log").display());
    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    let _ = launchctl(&["bootout", &format!("{}/{LABEL}", domain()?)]);
    let path = plist_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    println!("hearth no longer starts at login. Your apps and data are untouched.");
    Ok(())
}

pub fn status(listen: std::net::SocketAddr) -> anyhow::Result<()> {
    let out = launchctl(&["print", &format!("{}/{LABEL}", domain()?)])?;
    let text = String::from_utf8_lossy(&out.stdout);
    let state = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("state = "))
        .unwrap_or(if out.status.success() {
            "loaded"
        } else {
            "not installed"
        });
    println!("Login service: {state}");
    println!("Dashboard:     http://{listen}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_keeps_hearth_alive_with_logs() {
        let v = render_plist(
            Path::new("/opt/homebrew/bin/hearthd"),
            Some(Path::new("/c/hearth.toml")),
            Path::new("/l/hearthd.log"),
        );
        let mut buf = Vec::new();
        v.to_writer_xml(&mut buf).expect("xml");
        let xml = String::from_utf8(buf).expect("utf8");
        for needle in [
            "dev.hearth.hearthd",
            "<key>KeepAlive</key>",
            "/opt/homebrew/bin/hearthd",
            "--config",
            "/l/hearthd.log",
        ] {
            assert!(xml.contains(needle), "missing {needle}");
        }
    }
}
