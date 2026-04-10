//! Direct file logging — writes to %TEMP%\summit_debug.log.
//!
//! Use the `dlog!` macro instead of `log::debug!` for anything important.
//! This bypasses tauri_plugin_log entirely, so it always works even if the
//! plugin hasn't initialised or is buffering.
//!
//! Log location: %LOCALAPPDATA%\Temp\summit_debug.log
//! (typically C:\Users\<name>\AppData\Local\Temp\summit_debug.log)

use std::io::Write;

pub fn write(msg: &str) {
    let log_path = std::env::temp_dir().join("summit_debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        // chrono is already a dependency via tauri
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let pid = std::process::id();
        let _ = writeln!(f, "[{ts}] [pid:{pid}] {msg}");
    }
}

/// Log to %TEMP%\immich_debug.log AND to the standard log system.
///
/// Usage: `dlog!("message with {} args", value);`
#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {{
        let _msg = format!($($arg)*);
        $crate::debug_log::write(&_msg);
        log::info!("{}", _msg);
    }};
}
