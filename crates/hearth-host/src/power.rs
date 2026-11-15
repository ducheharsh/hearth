//! Power management: keep the Mac awake as a server, and read or apply the `pmset`
//! settings that matter for one (sleep on power, wake for network, restart after a
//! power failure, sleeping with the lid closed).

use serde::Serialize;

use crate::{CommandError, run, run_timeout};

/// What macOS will do on power, as far as a server cares.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PowerStatus {
    /// Running on the charger (always true on desktops).
    pub on_ac: bool,
    /// Battery percentage; `None` on Macs without a battery.
    pub battery: Option<u8>,
    /// Minutes until system sleep on power; `Some(0)` = never.
    pub sleep_on_ac: Option<u32>,
    /// Wake for network access on power.
    pub wake_on_lan: Option<bool>,
    /// Start up again after a power failure (desktops only; `None` when unsupported).
    pub restart_after_power_loss: Option<bool>,
    /// Sleep is disabled system-wide (keeps a closed laptop awake).
    pub sleep_disabled: bool,
    /// Some process holds a "prevent system sleep" assertion right now.
    pub sleep_prevented: bool,
}

impl PowerStatus {
    #[must_use]
    pub fn is_laptop(&self) -> bool {
        self.battery.is_some()
    }
}

/// Values from the `AC Power:` section of `pmset -g custom`.
fn ac_section(raw: &str) -> impl Iterator<Item = (&str, &str)> {
    raw.lines()
        .skip_while(|l| !l.starts_with("AC Power"))
        .skip(1)
        .take_while(|l| l.starts_with(' ') || l.starts_with('\t'))
        .filter_map(|l| {
            let l = l.trim();
            // Keys can contain spaces ("Sleep On Power Button"); the value is the last word.
            let (k, v) = l.rsplit_once(char::is_whitespace)?;
            Some((k.trim(), v.trim()))
        })
}

#[must_use]
pub fn parse_status(custom: &str, batt: &str, assertions: &str, live: &str) -> PowerStatus {
    let get = |key: &str| ac_section(custom).find(|(k, _)| *k == key).map(|(_, v)| v);
    let flag = |key: &str| get(key).map(|v| v == "1");
    let battery = batt
        .lines()
        .find(|l| l.contains("InternalBattery"))
        .and_then(|l| l.split('\t').nth(1).or_else(|| l.split_whitespace().nth(2)))
        .and_then(|s| s.split('%').next())
        .and_then(|p| p.trim().parse().ok());
    let assertion = |name: &str| {
        assertions
            .lines()
            .find(|l| l.trim_start().starts_with(name))
            .and_then(|l| l.split_whitespace().last())
            == Some("1")
    };
    PowerStatus {
        on_ac: batt.contains("'AC Power'") || battery.is_none(),
        battery,
        sleep_on_ac: get("sleep").and_then(|v| v.parse().ok()),
        wake_on_lan: flag("womp"),
        restart_after_power_loss: flag("autorestart"),
        sleep_disabled: live
            .lines()
            .any(|l| l.split_whitespace().collect::<Vec<_>>() == ["SleepDisabled", "1"]),
        sleep_prevented: assertion("PreventSystemSleep"),
    }
}

pub async fn status() -> Result<PowerStatus, CommandError> {
    let (custom, batt, assertions, live) = tokio::try_join!(
        run("pmset", &["-g", "custom"], &[]),
        run("pmset", &["-g", "batt"], &[]),
        run("pmset", &["-g", "assertions"], &[]),
        run("pmset", &["-g"], &[]),
    )?;
    Ok(parse_status(&custom, &batt, &assertions, &live))
}

/// Hold a "prevent system sleep" assertion for as long as `owner_pid` lives (macOS only
/// honours it on power, so a laptop on battery still sleeps normally). Kill the child
/// to release it.
pub fn caffeinate(owner_pid: u32) -> std::io::Result<tokio::process::Child> {
    tokio::process::Command::new(crate::resolve("caffeinate"))
        .args(["-s", "-w", &owner_pid.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
}

/// The `pmset` settings for a server on power. Only flags this Mac supports are set.
#[must_use]
pub fn recommended_args(s: &PowerStatus, lid_closed_ok: bool) -> Vec<String> {
    let mut args = vec!["-c", "sleep", "0", "disksleep", "0", "womp", "1"];
    if s.restart_after_power_loss.is_some() {
        args.extend(["autorestart", "1"]);
    }
    let mut out: Vec<String> = args.into_iter().map(str::to_owned).collect();
    if s.is_laptop() {
        out.push(";".into());
        out.extend(
            ["-a", "disablesleep", if lid_closed_ok { "1" } else { "0" }].map(str::to_owned),
        );
    }
    out
}

/// Apply settings with `pmset` through macOS's own administrator prompt (the user types
/// their password in a system dialog; hearth never sees it). Gives up after 2 minutes.
pub async fn apply_as_admin(args: &[String]) -> Result<(), CommandError> {
    let commands: String = args
        .split(|a| a == ";")
        .map(|chunk| format!("/usr/bin/pmset {}", chunk.join(" ")))
        .collect::<Vec<_>>()
        .join(" && ");
    // Arguments are fixed words and digits from `recommended_args`; nothing user-typed.
    debug_assert!(
        commands
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " /-&".contains(c))
    );
    let script = format!(
        "do shell script \"{commands}\" with administrator privileges with prompt \"hearth wants to keep this Mac running as a server.\""
    );
    run_timeout(
        "osascript",
        &["-e", &script],
        std::time::Duration::from_secs(120),
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUSTOM_LAPTOP: &str = "Battery Power:\n Sleep On Power Button 1\n womp                 0\n sleep                1\nAC Power:\n Sleep On Power Button 1\n womp                 1\n displaysleep         10\n sleep                1\n disksleep            10\n";
    const BATT: &str = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=20906083)\t100%; discharging; 9:34 remaining present: true\n";
    const ASSERT: &str = "Assertion status system-wide:\n   PreventSystemSleep             1\n   PreventUserIdleSystemSleep     1\n";

    #[test]
    fn reads_a_laptop() {
        let s = parse_status(CUSTOM_LAPTOP, BATT, ASSERT, " SleepDisabled\t\t0\n");
        assert_eq!(s.battery, Some(100));
        assert!(!s.on_ac && s.is_laptop());
        assert_eq!(s.sleep_on_ac, Some(1));
        assert_eq!(s.wake_on_lan, Some(true));
        assert_eq!(s.restart_after_power_loss, None);
        assert!(s.sleep_prevented && !s.sleep_disabled);
    }

    #[test]
    fn reads_a_desktop() {
        let custom = "AC Power:\n autorestart          1\n womp                 1\n sleep                0\n";
        let s = parse_status(
            custom,
            "Now drawing from 'AC Power'\n",
            "   PreventSystemSleep 0\n",
            " SleepDisabled\t\t1\n",
        );
        assert!(s.on_ac && !s.is_laptop());
        assert_eq!(s.sleep_on_ac, Some(0));
        assert_eq!(s.restart_after_power_loss, Some(true));
        assert!(s.sleep_disabled && !s.sleep_prevented);
    }

    #[test]
    fn recommends_only_supported_settings() {
        let laptop = parse_status(CUSTOM_LAPTOP, BATT, ASSERT, "");
        assert_eq!(
            recommended_args(&laptop, true).join(" "),
            "-c sleep 0 disksleep 0 womp 1 ; -a disablesleep 1"
        );
        let desktop = parse_status(
            "AC Power:\n autorestart 0\n",
            "Now drawing from 'AC Power'\n",
            "",
            "",
        );
        assert_eq!(
            recommended_args(&desktop, false).join(" "),
            "-c sleep 0 disksleep 0 womp 1 autorestart 1"
        );
    }
}
