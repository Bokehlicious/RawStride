use std::path::Path;
// Only the Windows eject code and the tests wait for a drive to disappear.
#[cfg(any(windows, test))]
use std::time::{Duration, Instant};

/// Returns the drive letter at the start of `source`, if present.
pub fn drive_of(source: &Path) -> Option<String> {
    let s = source.to_string_lossy();
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic() => {
            Some(format!("{}:", letter.to_ascii_uppercase()))
        }
        _ => None,
    }
}

/// PowerShell that ejects `drive` and stays alive until `<drive>\DCIM` is gone (at most 5 s):
/// the eject verb runs in the background and is cancelled if PowerShell exits at once.
#[cfg(any(windows, test))]
fn eject_script(drive: &str) -> String {
    format!(
        "$d = (New-Object -ComObject Shell.Application).Namespace(17).ParseName('{drive}'); \
         $d.InvokeVerb('Eject'); \
         for ($i = 0; $i -lt 20 -and (Test-Path '{drive}\\DCIM'); $i++) {{ \
             Start-Sleep -Milliseconds 250 \
         }}"
    )
}

/// Ejects the drive containing `source` on Windows.
pub fn eject(source: &Path) -> String {
    let Some(drive) = drive_of(source) else {
        return "Källan är ingen enhet, så inget matades ut.".into();
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::path::PathBuf;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const EJECT_ATTEMPTS: usize = 3;
        const EJECT_TIMEOUT: Duration = Duration::from_secs(5);
        const EJECT_POLL_INTERVAL: Duration = Duration::from_millis(250);

        let Some(system_root) = std::env::var_os("SystemRoot") else {
            return "Utmatning kunde inte startas: miljövariabeln SystemRoot saknas.".into();
        };
        let powershell = PathBuf::from(system_root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        let script = eject_script(&drive);
        let dcim = PathBuf::from(format!("{drive}\\DCIM"));
        match retry_until_gone(
            EJECT_ATTEMPTS,
            || {
                let status = std::process::Command::new(&powershell)
                    .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                    .creation_flags(CREATE_NO_WINDOW)
                    .status()
                    .map_err(|error| format!("{drive} kunde inte matas ut: {error}"))?;
                if !status.success() {
                    return Err(format!(
                        "{drive} kunde inte matas ut: PowerShell avslutades med felstatus {status}."
                    ));
                }
                Ok(())
            },
            || dcim.exists(),
            EJECT_TIMEOUT,
            EJECT_POLL_INTERVAL,
        ) {
            Ok(true) => format!("{drive} är utmatat och kan tas ut."),
            Ok(false) => format!(
                "{drive} kunde inte matas ut (används den av ett annat program?). Mata ut i Utforskaren. Antal försök: {EJECT_ATTEMPTS}."
            ),
            Err(error) => error,
        }
    }
    #[cfg(not(windows))]
    {
        format!("Utmatning av {drive} stöds bara på Windows.")
    }
}

#[cfg(any(windows, test))]
fn wait_until_gone(
    mut still_present: impl FnMut() -> bool,
    timeout: Duration,
    interval: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !still_present() {
            return true;
        }
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        std::thread::sleep(interval.min(deadline - now));
    }
}

#[cfg(any(windows, test))]
fn retry_until_gone(
    attempts: usize,
    mut invoke: impl FnMut() -> Result<(), String>,
    mut still_present: impl FnMut() -> bool,
    timeout: Duration,
    interval: Duration,
) -> Result<bool, String> {
    for _ in 0..attempts {
        invoke()?;
        if wait_until_gone(&mut still_present, timeout, interval) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eject_script_invokes_eject_and_waits_for_the_requested_drive() {
        let script = eject_script("E:");

        assert!(script.contains("ParseName('E:')"));
        assert!(script.contains("InvokeVerb('Eject')"));
        assert!(script.contains("Test-Path 'E:\\DCIM'"));
        assert!(script.contains("Start-Sleep"));
        assert_eq!(script.matches(':').count(), 2);
    }

    #[test]
    fn drive_letter_is_found_only_at_the_start() {
        assert_eq!(drive_of(Path::new(r"e:\DCIM")).as_deref(), Some("E:"));
        assert_eq!(drive_of(Path::new("F:")).as_deref(), Some("F:"));
        assert_eq!(drive_of(Path::new(r"\\server\share")), None);
        assert_eq!(drive_of(Path::new("/media/card")), None);
        assert_eq!(drive_of(Path::new("1:")), None);
        assert_eq!(drive_of(Path::new("")), None);
    }

    #[test]
    fn wait_until_gone_succeeds_when_already_gone() {
        let mut polls = 0;
        assert!(wait_until_gone(
            || {
                polls += 1;
                false
            },
            Duration::from_millis(20),
            Duration::from_millis(1)
        ));
        assert_eq!(polls, 1);
    }

    #[test]
    fn wait_until_gone_succeeds_after_a_few_polls() {
        let mut polls = 0;
        assert!(wait_until_gone(
            || {
                polls += 1;
                polls < 3
            },
            Duration::from_millis(20),
            Duration::from_millis(1)
        ));
        assert_eq!(polls, 3);
    }

    #[test]
    fn wait_until_gone_times_out_when_still_present() {
        assert!(!wait_until_gone(
            || true,
            Duration::from_millis(20),
            Duration::from_millis(1)
        ));
    }

    #[test]
    fn retry_until_gone_succeeds_after_first_attempt() {
        let invocations = std::cell::Cell::new(0);
        let result = retry_until_gone(
            3,
            || {
                invocations.set(invocations.get() + 1);
                Ok(())
            },
            || false,
            Duration::from_millis(2),
            Duration::from_millis(1),
        );

        assert_eq!(result, Ok(true));
        assert_eq!(invocations.get(), 1);
    }

    #[test]
    fn retry_until_gone_succeeds_after_second_attempt() {
        let invocations = std::cell::Cell::new(0);
        let result = retry_until_gone(
            3,
            || {
                invocations.set(invocations.get() + 1);
                Ok(())
            },
            || invocations.get() < 2,
            Duration::from_millis(2),
            Duration::from_millis(1),
        );

        assert_eq!(result, Ok(true));
        assert_eq!(invocations.get(), 2);
    }

    #[test]
    fn retry_until_gone_returns_false_after_all_attempts() {
        let invocations = std::cell::Cell::new(0);
        let result = retry_until_gone(
            3,
            || {
                invocations.set(invocations.get() + 1);
                Ok(())
            },
            || true,
            Duration::from_millis(2),
            Duration::from_millis(1),
        );

        assert_eq!(result, Ok(false));
        assert_eq!(invocations.get(), 3);
    }

    #[test]
    fn retry_until_gone_returns_invoke_error_without_retrying() {
        let invocations = std::cell::Cell::new(0);
        let result = retry_until_gone(
            3,
            || {
                invocations.set(invocations.get() + 1);
                Err("PowerShell failed".to_owned())
            },
            || true,
            Duration::from_millis(2),
            Duration::from_millis(1),
        );

        assert_eq!(result, Err("PowerShell failed".to_owned()));
        assert_eq!(invocations.get(), 1);
    }
}
