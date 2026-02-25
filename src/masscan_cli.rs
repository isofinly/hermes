use std::ffi::CString;
use std::marker::PhantomData;
use std::num::{NonZeroU16, NonZeroU64};
use std::os::raw::c_char;
use std::process::{Command, Stdio};

use crate::masscan_api::raw::masscan_cli_main;

#[derive(Debug)]
pub enum MasscanError {
    EmptyValue(&'static str),
    InvalidPortRange { start: u16, end: u16 },
    ArgumentContainsNul(String),
    NonZeroExit(i32),
    SpawnFailed(String),
    InvalidUtf8Output(String),
}

impl std::fmt::Display for MasscanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyValue(field) => write!(f, "{field} must not be empty"),
            Self::InvalidPortRange { start, end } => {
                write!(f, "invalid port range {start}-{end}: start must be <= end")
            }
            Self::ArgumentContainsNul(value) => write!(f, "argument contains NUL byte: {value}"),
            Self::NonZeroExit(code) => write!(
                f,
                "masscan returned non-zero exit code: {code}. If this is a permission error, retry under sudo"
            ),
            Self::SpawnFailed(message) => {
                write!(f, "failed to spawn masscan subprocess: {message}")
            }
            Self::InvalidUtf8Output(message) => {
                write!(f, "masscan produced non-UTF8 stdout: {message}")
            }
        }
    }
}

impl std::error::Error for MasscanError {}

#[derive(Clone, Debug)]
pub struct NonEmptyList<T> {
    first: T,
    rest: Vec<T>,
}

impl<T> NonEmptyList<T> {
    pub fn new(first: T) -> Self {
        Self {
            first,
            rest: Vec::new(),
        }
    }

    pub fn push(mut self, value: T) -> Self {
        self.rest.push(value);
        self
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }
}

#[derive(Clone, Debug)]
pub struct TargetSpec(String);

impl TargetSpec {
    pub fn new(value: impl Into<String>) -> Result<Self, MasscanError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(MasscanError::EmptyValue("target"));
        }
        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub enum PortSpec {
    Single(NonZeroU16),
    Range { start: NonZeroU16, end: NonZeroU16 },
}

impl PortSpec {
    pub fn single(port: NonZeroU16) -> Self {
        Self::Single(port)
    }

    pub fn range(start: NonZeroU16, end: NonZeroU16) -> Result<Self, MasscanError> {
        if start.get() > end.get() {
            return Err(MasscanError::InvalidPortRange {
                start: start.get(),
                end: end.get(),
            });
        }

        Ok(Self::Range { start, end })
    }

    fn as_cli_fragment(&self) -> String {
        match self {
            Self::Single(port) => port.get().to_string(),
            Self::Range { start, end } => format!("{}-{}", start.get(), end.get()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PortSelection(NonEmptyList<PortSpec>);

impl PortSelection {
    pub fn new(first: PortSpec) -> Self {
        Self(NonEmptyList::new(first))
    }

    pub fn push(mut self, item: PortSpec) -> Self {
        self.0 = self.0.push(item);
        self
    }

    fn as_cli_value(&self) -> String {
        self.0
            .iter()
            .map(PortSpec::as_cli_fragment)
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Debug)]
pub struct ModeUnset;

#[derive(Debug)]
pub struct ScanMode;

#[derive(Debug)]
pub struct ReadScanMode;

#[derive(Debug)]
pub struct MasscanCommand<Mode> {
    args: Vec<String>,
    _mode: PhantomData<Mode>,
}

impl Default for MasscanCommand<ModeUnset> {
    fn default() -> Self {
        Self::new()
    }
}

impl MasscanCommand<ModeUnset> {
    pub fn new() -> Self {
        Self {
            args: vec!["masscan".to_string()],
            _mode: PhantomData,
        }
    }

    pub fn scan(
        targets: NonEmptyList<TargetSpec>,
        ports: PortSelection,
    ) -> MasscanCommand<ScanMode> {
        let mut command = Self::new().option("--ports", ports.as_cli_value());

        for target in targets.iter() {
            command = command.arg(target.as_str());
        }

        MasscanCommand {
            args: command.args,
            _mode: PhantomData,
        }
    }

    pub fn readscan(path: impl Into<String>) -> Result<MasscanCommand<ReadScanMode>, MasscanError> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(MasscanError::EmptyValue("readscan path"));
        }

        let command = Self::new().option("--readscan", path);
        Ok(MasscanCommand {
            args: command.args,
            _mode: PhantomData,
        })
    }
}

impl<Mode> MasscanCommand<Mode> {
    pub fn arg(mut self, value: impl Into<String>) -> Self {
        self.args.push(value.into());
        self
    }

    pub fn flag(self, name: impl Into<String>) -> Self {
        self.arg(name)
    }

    pub fn option(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.args.push(name.into());
        self.args.push(value.into());
        self
    }

    pub fn rate(self, packets_per_second: NonZeroU64) -> Self {
        self.option("--rate", packets_per_second.get().to_string())
    }

    pub fn max_retries(self, retries: u32) -> Self {
        self.option("--max-retries", retries.to_string())
    }

    pub fn wait(self, seconds: u32) -> Self {
        self.option("--wait", seconds.to_string())
    }

    pub fn output_ndjson(self, path: impl Into<String>) -> Result<Self, MasscanError> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(MasscanError::EmptyValue("output path"));
        }
        Ok(self.option("-oD", path))
    }

    fn invoke_inner(&self) -> Result<(), MasscanError> {
        let cstrings: Vec<CString> = self
            .args
            .iter()
            .map(|arg| {
                CString::new(arg.as_str())
                    .map_err(|_| MasscanError::ArgumentContainsNul(arg.clone()))
            })
            .collect::<Result<_, _>>()?;

        let mut argv: Vec<*mut c_char> = cstrings
            .iter()
            .map(|arg| arg.as_ptr() as *mut c_char)
            .collect();
        argv.push(std::ptr::null_mut());

        // SAFETY: The C entrypoint follows masscan's CLI contract (`argc`/`argv`)
        let exit_code = unsafe { masscan_cli_main(cstrings.len() as i32, argv.as_mut_ptr()) };

        if exit_code != 0 {
            return Err(MasscanError::NonZeroExit(exit_code));
        }

        Ok(())
    }

    pub fn invoke_subprocess_capture_stdout(&self) -> Result<String, MasscanError> {
        let program = self
            .args
            .first()
            .ok_or_else(|| MasscanError::SpawnFailed("missing executable name".to_string()))?;

        let output = Command::new(program)
            .args(self.args.iter().skip(1))
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|err| MasscanError::SpawnFailed(err.to_string()))?;

        if !output.status.success() {
            return Err(MasscanError::NonZeroExit(
                output.status.code().unwrap_or(-1),
            ));
        }

        String::from_utf8(output.stdout)
            .map_err(|err| MasscanError::InvalidUtf8Output(err.to_string()))
    }
}

impl MasscanCommand<ScanMode> {
    pub fn invoke(&self) -> Result<(), MasscanError> {
        self.invoke_inner()
    }
}

impl MasscanCommand<ReadScanMode> {
    pub fn invoke(&self) -> Result<(), MasscanError> {
        self.invoke_inner()
    }
}
