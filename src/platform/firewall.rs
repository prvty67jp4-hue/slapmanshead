//! Windows Firewall: can friends' packets reach this game when it hosts? A read-only check, and
//! a fix the person starts with a button (Windows asks them for permission first).

/// What the firewall does with players knocking on our port.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FwStatus {
    /// The firewall is off.
    Off,
    /// A rule lets the game (or the port) in.
    Allowed,
    /// A rule blocks the game (often from clicking "Cancel" on Windows' first prompt).
    Blocked,
    /// No rule yet: Windows will ask the first time someone joins, or silently block.
    NoRule,
    /// Couldn't tell (not Windows, or the check failed).
    Unknown,
}

impl FwStatus {
    pub fn text(self) -> &'static str {
        match self {
            FwStatus::Off => "Windows Firewall is off: friends can reach you",
            FwStatus::Allowed => "Windows Firewall lets friends in",
            FwStatus::Blocked => "Windows Firewall is BLOCKING Duel Rush: friends can't join",
            FwStatus::NoRule => "Windows Firewall has no rule for Duel Rush yet: friends may be blocked",
            FwStatus::Unknown => "Couldn't check Windows Firewall",
        }
    }
    pub fn ok(self) -> bool {
        matches!(self, FwStatus::Off | FwStatus::Allowed)
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
/// PowerShell's -EncodedCommand wants the script as base64 of UTF-16LE (so paths with spaces or
/// quotes need no escaping).
fn encode_script(script: &str) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Is there a firewall here the game knows how to check (Windows Firewall)? Elsewhere the host
/// screen doesn't mention one.
pub fn applies() -> bool {
    cfg!(windows)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn exe_path() -> Option<String> {
    std::env::current_exe().ok().map(|p| p.to_string_lossy().replace('\'', "''"))
}

#[cfg(windows)]
fn powershell(script: &str) -> Option<std::process::Output> {
    use std::os::windows::process::CommandExt;
    const NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encode_script(script)])
        .creation_flags(NO_WINDOW)
        .output()
        .ok()
}

/// Look at the firewall's rules for this program and the port (reads only; takes a second or two).
pub fn check(port: u16) -> FwStatus {
    #[cfg(windows)]
    {
        let Some(exe) = exe_path() else { return FwStatus::Unknown };
        let script = format!(
            r#"$ErrorActionPreference = 'SilentlyContinue'
$exe = '{exe}'; $port = '{port}'
if (-not (Get-NetFirewallProfile | Where-Object {{ $_.Enabled -eq 'True' }})) {{ 'OFF'; exit }}
$r = @()
foreach ($f in @(Get-NetFirewallApplicationFilter -Program $exe)) {{
  $rule = $f | Get-NetFirewallRule
  $pf = $rule | Get-NetFirewallPortFilter
  if ($pf.Protocol -in @('UDP', 'Any')) {{ $r += $rule }}
}}
foreach ($pf in @(Get-NetFirewallPortFilter -Protocol UDP)) {{ if ($pf.LocalPort -contains $port) {{ $r += ($pf | Get-NetFirewallRule) }} }}
$r = @($r | Where-Object {{ $_.Direction -eq 'Inbound' -and $_.Enabled -eq 'True' }})
if ($r | Where-Object {{ $_.Action -eq 'Block' }}) {{ 'BLOCK' }} elseif ($r | Where-Object {{ $_.Action -eq 'Allow' }}) {{ 'ALLOW' }} else {{ 'NONE' }}
"#
        );
        let Some(out) = powershell(&script) else { return FwStatus::Unknown };
        let text = String::from_utf8_lossy(&out.stdout);
        match text.trim() {
            "OFF" => FwStatus::Off,
            "ALLOW" => FwStatus::Allowed,
            "BLOCK" => FwStatus::Blocked,
            "NONE" => FwStatus::NoRule,
            _ => FwStatus::Unknown,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = port;
        FwStatus::Unknown
    }
}

/// Let players in: remove rules blocking this program and add rules allowing it and the UDP
/// port. Windows shows its permission prompt first; this waits for it.
pub fn allow(port: u16) -> Result<(), String> {
    #[cfg(windows)]
    {
        let exe = exe_path().ok_or("couldn't find the game's file")?;
        let fix = format!(
            r#"$exe = '{exe}'
Get-NetFirewallApplicationFilter -Program $exe -ErrorAction SilentlyContinue | Get-NetFirewallRule | Where-Object {{ $_.Direction -eq 'Inbound' -and $_.Action -eq 'Block' }} | Remove-NetFirewallRule
Get-NetFirewallRule -DisplayName 'Duel Rush*' -ErrorAction SilentlyContinue | Remove-NetFirewallRule
New-NetFirewallRule -DisplayName 'Duel Rush (UDP {port})' -Direction Inbound -Protocol UDP -LocalPort {port} -Action Allow -Profile Any | Out-Null
New-NetFirewallRule -DisplayName 'Duel Rush (game)' -Direction Inbound -Program $exe -Protocol UDP -Action Allow -Profile Any | Out-Null
"#
        );
        // ask Windows to run the fix with permission (it shows its prompt), and wait
        let launcher = format!(
            "$p = Start-Process powershell -Verb RunAs -Wait -PassThru -WindowStyle Hidden -ArgumentList '-NoProfile','-NonInteractive','-EncodedCommand','{}'; exit $p.ExitCode",
            encode_script(&fix)
        );
        let out = powershell(&launcher).ok_or("couldn't start PowerShell")?;
        if out.status.success() {
            Ok(())
        } else {
            Err("Windows didn't give permission (or the change failed)".into())
        }
    }
    #[cfg(not(windows))]
    {
        let _ = port;
        Err("only needed on Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_values() {
        // "hi" as UTF-16LE is 68 00 69 00
        assert_eq!(encode_script("hi"), "aABpAA==");
        assert_eq!(encode_script("a"), "YQA=");
    }

    /// Reads the firewall (no changes): cargo test -- --ignored firewall_check
    #[test]
    #[ignore]
    fn firewall_check_runs() {
        let s = check(27960);
        assert_ne!(s, FwStatus::Unknown, "the check answers");
    }
}
