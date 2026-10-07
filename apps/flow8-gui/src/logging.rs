use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const MAX_PRODUCTION_LOG_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RETAINED_PRODUCTION_LOGS: usize = 6;
const MAX_RETAINED_PRODUCTION_LOG_BYTES: u64 = 96 * 1024 * 1024;
const PRODUCTION_LOG_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const LOG_CLEANUP_INTERVAL: Duration = Duration::from_secs(60 * 60);
const PRODUCTION_LOG_LIMIT_NOTICE: &[u8] =
    b"warning: FLOW 8 log reached its 16 MiB limit; further entries are suppressed until the next launch\n";

pub(super) struct LogMaintenance {
    stop: mpsc::Sender<()>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for LogMaintenance {
    fn drop(&mut self) {
        // Wake the worker immediately instead of waiting for the hourly timer.
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn is_production_log_name(name: &str) -> bool {
    let Some(fields) = name
        .strip_prefix("windows-production-connection-")
        .and_then(|name| name.strip_suffix(".log"))
    else {
        return false;
    };
    let mut fields = fields.split('-');
    let valid_number = |field: Option<&str>| {
        field.is_some_and(|field| {
            !field.is_empty()
                && field.bytes().all(|byte| byte.is_ascii_digit())
                && field.parse::<u128>().is_ok()
        })
    };
    valid_number(fields.next())
        && valid_number(fields.next())
        && valid_number(fields.next())
        && fields.next().is_none()
}

fn cleanup_production_logs(directory: &Path, active_file: Option<&Path>) -> io::Result<usize> {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "log directory is not a real directory",
        ));
    }

    let now = SystemTime::now();
    let mut logs = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || !entry
                .file_name()
                .to_str()
                .is_some_and(is_production_log_name)
        {
            continue;
        }
        let metadata = entry.metadata()?;
        logs.push((
            entry.path(),
            metadata.modified().unwrap_or(now),
            metadata.len(),
        ));
    }
    logs.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let mut remaining = logs.len();
    let mut bytes = logs
        .iter()
        .fold(0_u64, |total, log| total.saturating_add(log.2));
    let mut removed = 0;
    for (path, modified, size) in logs {
        if active_file == Some(path.as_path()) {
            continue;
        }
        let expired = now
            .duration_since(modified)
            .is_ok_and(|age| age >= PRODUCTION_LOG_RETENTION);
        if expired
            || remaining > MAX_RETAINED_PRODUCTION_LOGS
            || bytes > MAX_RETAINED_PRODUCTION_LOG_BYTES
        {
            // Other application instances hold this lock for the lifetime of
            // their detailed log. Never remove a file that is still in use.
            let lease = match OpenOptions::new().read(true).write(true).open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            match lease.try_lock() {
                Ok(()) => {}
                Err(fs::TryLockError::WouldBlock) => continue,
                Err(fs::TryLockError::Error(error)) => return Err(error),
            }
            match fs::remove_file(&path) {
                Ok(()) => removed += 1,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            remaining = remaining.saturating_sub(1);
            bytes = bytes.saturating_sub(size);
        }
    }
    Ok(removed)
}

fn maintain_production_logs(directory: &Path, active_file: Option<&Path>) {
    match cleanup_production_logs(directory, active_file) {
        Ok(0) => {}
        Ok(removed) => {
            tracing::info!(target: "flow8_gui", removed, "old FLOW 8 diagnostic logs removed")
        }
        Err(error) => {
            tracing::warn!(target: "flow8_gui", %error, "could not clean up old FLOW 8 diagnostic logs")
        }
    }
}

fn start_log_maintenance(active_file: Option<PathBuf>) -> Option<LogMaintenance> {
    let directory = match production_log_directory() {
        Ok(directory) => directory,
        Err(error) => {
            tracing::warn!(target: "flow8_gui", %error, "FLOW 8 diagnostic log cleanup unavailable");
            return None;
        }
    };
    maintain_production_logs(&directory, active_file.as_deref());
    let (stop, stopped) = mpsc::channel();
    match thread::Builder::new()
        .name("flow8-log-cleanup".into())
        .spawn(move || {
            while matches!(
                stopped.recv_timeout(LOG_CLEANUP_INTERVAL),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                maintain_production_logs(&directory, active_file.as_deref());
            }
        }) {
        Ok(worker) => Some(LogMaintenance {
            stop,
            worker: Some(worker),
        }),
        Err(error) => {
            tracing::warn!(target: "flow8_gui", %error, "could not start FLOW 8 diagnostic log cleanup timer");
            None
        }
    }
}

struct ProductionLogFile {
    file: File,
    written: u64,
    capped: bool,
}

#[derive(Clone)]
struct ProductionLogWriter {
    file: Arc<Mutex<ProductionLogFile>>,
    console: bool,
}

struct ProductionLogSink {
    file: Arc<Mutex<ProductionLogFile>>,
    stdout: Option<io::Stdout>,
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for ProductionLogWriter {
    type Writer = ProductionLogSink;

    fn make_writer(&'writer self) -> Self::Writer {
        ProductionLogSink {
            file: Arc::clone(&self.file),
            stdout: self.console.then(io::stdout),
        }
    }
}

impl Write for ProductionLogSink {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if let Some(stdout) = &mut self.stdout {
            let _ = stdout.write_all(buffer);
        }
        let mut log = self.file.lock().unwrap_or_else(|error| error.into_inner());
        if !log.capped {
            if log.written.saturating_add(buffer.len() as u64)
                <= MAX_PRODUCTION_LOG_BYTES - PRODUCTION_LOG_LIMIT_NOTICE.len() as u64
            {
                log.file.write_all(buffer)?;
                log.written += buffer.len() as u64;
            } else {
                log.capped = true;
                let _ = log.file.write_all(PRODUCTION_LOG_LIMIT_NOTICE);
                if self.stdout.is_some() {
                    eprintln!("warning: FLOW 8 log reached its 16 MiB limit");
                }
            }
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .file
            .flush()?;
        if let Some(stdout) = &mut self.stdout {
            let _ = stdout.flush();
        }
        Ok(())
    }
}

fn production_log_directory() -> io::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(root)
            .join("FLOW 8 PC Controller")
            .join("logs"));
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(root) = std::env::var_os("XDG_STATE_HOME") {
            return Ok(PathBuf::from(root).join("flow8-pc-controller"));
        }
        if let Some(root) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(root).join(".local/state/flow8-pc-controller"));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no private per-user log directory is available",
    ))
}

fn prepare_log_directory() -> io::Result<PathBuf> {
    let directory = production_log_directory()?;
    fs::create_dir_all(&directory)?;
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "log directory is not a real directory",
        ));
    }
    #[cfg(unix)]
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    Ok(directory)
}

fn open_unique_log(
    directory: &Path,
    prefix: &str,
    lock: bool,
    exhausted_message: &'static str,
) -> io::Result<(File, PathBuf)> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..16 {
        let path = directory.join(format!(
            "{prefix}-{timestamp}-{}-{attempt}.log",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&path) {
            Ok(file) => {
                if lock {
                    file.lock()?;
                }
                return Ok((file, path));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        exhausted_message,
    ))
}

fn open_production_log() -> io::Result<(File, PathBuf)> {
    let directory = prepare_log_directory()?;
    open_unique_log(
        &directory,
        "windows-production-connection",
        true,
        "could not allocate a unique FLOW 8 log filename",
    )
}

// Routine release logs rotate independently of hardware-evidence captures.
fn open_application_log() -> io::Result<(File, PathBuf)> {
    let directory = prepare_log_directory()?;
    const MAX_RETAINED_APPLICATION_LOGS: usize = 12;
    let mut old_logs = Vec::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("flow8-gui-") && name.ends_with(".log") && entry.file_type()?.is_file()
        {
            old_logs.push(entry.path());
        }
    }
    old_logs.sort();
    let remove_count = old_logs
        .len()
        .saturating_sub(MAX_RETAINED_APPLICATION_LOGS - 1);
    for path in old_logs.into_iter().take(remove_count) {
        fs::remove_file(path)?;
    }

    open_unique_log(
        &directory,
        "flow8-gui",
        false,
        "could not allocate a unique FLOW 8 application log filename",
    )
}

pub(super) fn flow8_ble_verbose_logging(filter: &str) -> bool {
    filter.split(',').any(|directive| {
        let directive = directive.trim().to_ascii_lowercase();
        matches!(
            directive.as_str(),
            "flow8_ble=debug"
                | "flow8_ble=trace"
                | "flow8_directhci=debug"
                | "flow8_directhci=trace"
        )
    })
}

pub(super) fn init_tracing() -> (Option<String>, Option<LogMaintenance>) {
    const DEFAULT_FILTER: &str = "flow8_ble=info,flow8_directhci=info,flow8_gui=info";
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_FILTER.into());
    let filter = tracing_subscriber::EnvFilter::try_new(&rust_log)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_FILTER));
    let verbose = flow8_ble_verbose_logging(&rust_log);
    let console = !cfg!(all(windows, not(debug_assertions)));
    let log_file = if verbose {
        Some(open_production_log())
    } else if !console {
        Some(open_application_log())
    } else {
        None
    };
    let mut startup_error = None;
    if let Some(log_file) = log_file {
        match log_file {
            Ok((file, path)) => {
                if console {
                    eprintln!("FLOW 8 log: {}", path.display());
                }
                let _ = tracing_subscriber::fmt()
                    .with_env_filter(filter)
                    .with_ansi(false)
                    .with_writer(ProductionLogWriter {
                        file: Arc::new(Mutex::new(ProductionLogFile {
                            file,
                            written: 0,
                            capped: false,
                        })),
                        console,
                    })
                    .try_init();
                tracing::info!(target: "flow8_ble", path = %path.display(), "FLOW 8 log file opened");
                if verbose {
                    tracing::warn!(
                        target: "flow8_ble",
                        "verbose logs contain raw device state and device identifiers; review before sharing"
                    );
                }
                return (None, start_log_maintenance(Some(path)));
            }
            Err(error) => {
                let message = format!("could not create private FLOW 8 log: {error}");
                if console {
                    eprintln!("warning: {message}");
                } else {
                    startup_error = Some(message);
                }
            }
        }
    }
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
    (startup_error, start_log_maintenance(None))
}
