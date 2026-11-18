//! Opens a file in Neovim, in an iTerm tab. There is one Neovim per repo, reached through a socket, so
//! its language servers start only once.

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `line` counts from 1.
pub fn open(root: &Path, path: &str, line: usize) -> Result<(), String> {
    let socket = socket(root);
    let file = root.join(path);
    let session_file = socket.with_extension("iterm");
    if UnixStream::connect(&socket).is_ok() {
        edit(&socket, &file, line)?;
        let session = std::fs::read_to_string(&session_file).unwrap_or_default();
        if osascript(&select_script(session.trim()))? != "true" {
            return Err("Neovim for this repo runs outside iTerm".to_owned());
        }
    } else {
        // Left behind by a Neovim that didn't exit cleanly
        let _ = std::fs::remove_file(&socket);
        let command = start_command(root, &socket, &file, line);
        let session = osascript(&new_tab_script(&command))?;
        std::fs::write(&session_file, session)
            .map_err(|e| format!("Can't save the iTerm session: {e}"))?;
    }
    Ok(())
}

fn edit(socket: &Path, file: &Path, line: usize) -> Result<(), String> {
    run(
        "nvim",
        &[
            "--server",
            &socket.to_string_lossy(),
            "--remote-expr",
            &edit_expression(file, line),
        ],
    )?;
    Ok(())
}

/// `fnameescape` handles spaces and other special characters in the path.
fn edit_expression(file: &Path, line: usize) -> String {
    let file = file.to_string_lossy().replace('\'', "''");
    format!("execute('edit +{line} ' .. fnameescape('{file}'))")
}

/// Typed into the shell of the new tab, so Neovim gets the user's PATH and environment. The leading
/// space keeps it out of the shell history, and `exec` closes the tab when Neovim exits.
fn start_command(root: &Path, socket: &Path, file: &Path, line: usize) -> String {
    format!(
        " cd {} && exec nvim --listen {} +{line} {}",
        shell_quote(&root.to_string_lossy()),
        shell_quote(&socket.to_string_lossy()),
        shell_quote(&file.to_string_lossy()),
    )
}

/// Works in sh, zsh and fish.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Returns the id of the new session.
fn new_tab_script(command: &str) -> String {
    format!(
        r#"tell application "iTerm"
    activate
    if (count of windows) is 0 then
        set newSession to current session of (create window with default profile)
    else
        tell current window to set newSession to current session of (create tab with default profile)
    end if
    tell newSession to write text {}
    return id of newSession
end tell"#,
        applescript_string(command)
    )
}

/// Returns `true` when the session is found.
fn select_script(session: &str) -> String {
    format!(
        r#"tell application "iTerm"
    repeat with aWindow in windows
        repeat with aTab in tabs of aWindow
            repeat with aSession in sessions of aTab
                if id of aSession is {} then
                    select aWindow
                    tell aTab to select
                    tell aSession to select
                    activate
                    return true
                end if
            end repeat
        end repeat
    end repeat
    return false
end tell"#,
        applescript_string(session)
    )
}

fn applescript_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', r"\\").replace('"', "\\\""))
}

/// In the temp folder, named after the repo root so each worktree gets its own Neovim.
fn socket(root: &Path) -> PathBuf {
    std::env::temp_dir().join(format!("jetdiff-nvim-{:016x}.sock", fnv1a(root)))
}

/// Stable across builds, unlike `DefaultHasher`, so a running Neovim is found after an update.
fn fnv1a(root: &Path) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    root.to_string_lossy().bytes().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

fn osascript(script: &str) -> Result<String, String> {
    run("osascript", &["-e", script])
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Can't run {program}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{program} failed: {}", stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_socket_per_root() {
        let main = socket(Path::new("/git/jetdiff"));
        assert_eq!(main, socket(Path::new("/git/jetdiff")));
        assert_ne!(main, socket(Path::new("/git/worktrees/jetdiff-x")));
    }

    #[test]
    fn path_is_quoted_for_vim() {
        assert_eq!(
            edit_expression(Path::new("/a/it's.kt"), 12),
            "execute('edit +12 ' .. fnameescape('/a/it''s.kt'))"
        );
    }

    #[test]
    fn start_command_is_quoted_for_the_shell() {
        let command = start_command(
            Path::new("/git/my repo"),
            Path::new("/tmp/a.sock"),
            Path::new("/git/my repo/it's.kt"),
            7,
        );
        assert_eq!(
            command,
            r" cd '/git/my repo' && exec nvim --listen '/tmp/a.sock' +7 '/git/my repo/it'\''s.kt'"
        );
    }

    #[test]
    fn applescript_string_escapes_quotes_and_backslashes() {
        assert_eq!(applescript_string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    }
}
