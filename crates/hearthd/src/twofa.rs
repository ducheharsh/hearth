//! Two-factor enrollment for gateway logins (TOTP via Authelia).
//!
//! Authelia generates and stores the secret (`authelia storage user totp generate`);
//! hearth shows it as a QR code and verifies one code from the user's authenticator
//! app before marking the login as enrolled, so a bad scan can't lock anyone out.

use anyhow::{Context as _, bail};
use hearth_host::compose::{ComposeEnv, Project};
use qrcode::QrCode;
use qrcode::render::svg;
use serde::Serialize;
use totp_rs::{Algorithm, Builder, Secret, Totp};

use crate::events::Level;
use crate::state::SharedState;
use crate::{gateway, store};

#[derive(Debug, Serialize)]
pub struct Enrollment {
    pub uri: String,
    pub secret: String,
    /// Inline SVG of the QR code.
    pub qr_svg: String,
}

fn authelia(st: &SharedState) -> Project {
    let d = st.config.gateway_dir();
    Project {
        name: "hearth-gateway".into(),
        files: vec![d.join("compose.yml")],
        dir: d,
    }
}

async fn cli(st: &SharedState, args: &[&str]) -> anyhow::Result<String> {
    if !st.settings.read().await.gateway_enabled {
        bail!("Turn on the gateway first: 2FA is stored by its login service.");
    }
    // The storage CLI doesn't read the key from the environment, so pass it explicitly.
    let env_file = std::fs::read_to_string(st.config.gateway_dir().join("authelia.env"))
        .context("gateway secrets missing")?;
    let key = env_file
        .lines()
        .find_map(|l| l.strip_prefix("AUTHELIA_STORAGE_ENCRYPTION_KEY="))
        .context("storage key missing")?
        .trim()
        .to_owned();
    let mut cmd = vec!["authelia", "storage", "user", "totp"];
    cmd.extend_from_slice(args);
    cmd.extend([
        "--encryption-key",
        key.as_str(),
        "--config",
        "/config/configuration.yml",
    ]);
    let env = ComposeEnv::default().with("TS_HOST", st.ts_host().await);
    Ok(authelia(st).exec(&env, "authelia", &cmd).await?)
}

/// Pull `otpauth://…` out of Authelia's CLI output.
fn find_uri(out: &str) -> Option<String> {
    let start = out.find("otpauth://")?;
    let rest = &out[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '\'' || c == '"')
        .unwrap_or(rest.len());
    Some(rest[..end].to_owned())
}

fn secret_of(uri: &str) -> Option<String> {
    let q = uri.split_once('?')?.1;
    q.split('&')
        .find_map(|kv| kv.strip_prefix("secret="))
        .map(str::to_owned)
}

fn user_exists(st: &SharedState, name: &str) -> anyhow::Result<()> {
    if gateway::load_users(st)?.iter().any(|u| u.username == name) {
        Ok(())
    } else {
        bail!("No login named {name}.")
    }
}

/// Create (or replace) the TOTP secret for `user` and return the QR code to scan.
pub async fn start(st: &SharedState, user: &str) -> anyhow::Result<Enrollment> {
    user_exists(st, user)?;
    let out = cli(st, &["generate", user, "--force"])
        .await
        .context("generating the secret")?;
    let uri = find_uri(&out).context("Authelia didn't return a setup link")?;
    let secret = secret_of(&uri).context("setup link has no secret")?;
    let qr_svg = QrCode::new(uri.as_bytes())?
        .render::<svg::Color<'_>>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .build();
    set_enrolled(st, user, false)?;
    Ok(Enrollment {
        uri,
        secret,
        qr_svg,
    })
}

/// Check a 6-digit code against the pending secret; on success mark the login enrolled.
pub async fn verify(st: &SharedState, user: &str, secret: &str, code: &str) -> anyhow::Result<()> {
    user_exists(st, user)?;
    let secret = Secret::try_from_base32(secret).map_err(|_| anyhow::anyhow!("invalid secret"))?;
    let totp = authelia_totp(secret);
    let code: String = code.chars().filter(char::is_ascii_digit).collect();
    if code.len() != 6 || totp.check_current(&code).is_none() {
        bail!("That code doesn't match. Check the time on your phone and try the newest code.");
    }
    set_enrolled(st, user, true)?;
    st.event(
        Level::Success,
        "gateway",
        "Two-factor enabled",
        format!("{user} now signs in with an authenticator app."),
    )
    .await;
    Ok(())
}

pub async fn remove(st: &SharedState, user: &str) -> anyhow::Result<()> {
    user_exists(st, user)?;
    cli(st, &["delete", user])
        .await
        .context("removing the secret")?;
    set_enrolled(st, user, false)?;
    st.event(
        Level::Warning,
        "gateway",
        "Two-factor removed",
        format!("{user} will set it up again at next sign-in."),
    )
    .await;
    Ok(())
}

fn set_enrolled(st: &SharedState, user: &str, on: bool) -> anyhow::Result<()> {
    let mut users = gateway::load_users(st)?;
    for u in users.iter_mut().filter(|u| u.username == user) {
        u.totp = on;
    }
    store::save(&st.config.gateway_dir().join("users.json"), &users)
}

/// Authelia's defaults: SHA1, 6 digits, 30 s period; allow ±1 step of clock drift.
fn authelia_totp(secret: Secret) -> Totp {
    Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(secret)
        .build_noncompliant()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_authelia_output() {
        let out = "Successfully generated TOTP configuration for user 'harsh' with URI 'otpauth://totp/hearth:harsh?algorithm=SHA1&digits=6&issuer=hearth&period=30&secret=JBSWY3DPEHPK3PXP'\n";
        let uri = find_uri(out).expect("uri");
        assert!(uri.ends_with("secret=JBSWY3DPEHPK3PXP"));
        assert_eq!(secret_of(&uri).as_deref(), Some("JBSWY3DPEHPK3PXP"));
    }

    #[test]
    fn totp_math_matches_rfc6238() {
        // RFC 6238 test secret "12345678901234567890" (SHA1) at T=59 → 94287082 (8 digits).
        let t = Builder::new()
            .with_algorithm(Algorithm::SHA1)
            .with_digits(8)
            .with_step_duration(30)
            .with_secret(b"12345678901234567890".to_vec())
            .build_noncompliant();
        assert_eq!(t.generate(59).to_string(), "94287082");
        let six = authelia_totp(Secret::try_from_base32("JBSWY3DPEHPK3PXP").expect("b32"));
        assert_eq!(six.generate(59).to_string().len(), 6);
    }
}
