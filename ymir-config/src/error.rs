use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use miette::Diagnostic;

#[derive(Debug)]
pub struct ConfigParseResult<T, E> {
    pub config: Result<T, E>,

    // We always try to return includes for the file watcher.
    //
    // If the main config is valid, but an included file fails to parse, config will be an Err(),
    // but includes will still be filled, so that fixing just the included file is enough to
    // trigger a reload.
    pub includes: Vec<PathBuf>,
}

/// An error that occurs while parsing a Lua config program.
///
/// This is either a Lua evaluation error (the traceback text) or a collection of validation
/// diagnostics (one `section.key: message` line per offending key).
#[derive(Debug)]
pub enum ConfigError {
    /// A Lua runtime error; `path` is the config file (or include) that failed to evaluate.
    Lua { path: PathBuf, message: String },
    /// One or more validation errors, e.g. `input.keyboard.repeat-rate: expected a number`.
    Validation { path: PathBuf, messages: Vec<String> },
}

impl ConfigError {
    pub fn runtime(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Lua {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn validation(path: impl Into<PathBuf>, messages: Vec<String>) -> Self {
        Self::Validation {
            path: path.into(),
            messages,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Lua { path, .. } | Self::Validation { path, .. } => Some(path),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lua { path, message } => {
                write!(f, "error in {:?}:\n{message}", path.display())
            }
            Self::Validation { path, messages } => {
                write!(f, "error in {:?}:", path.display())?;
                for message in messages {
                    write!(f, "\n  {message}")?;
                }
                Ok(())
            }
        }
    }
}

impl Error for ConfigError {}

impl Diagnostic for ConfigError {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(match self {
            Self::Lua { .. } => "ymir::config::lua",
            Self::Validation { .. } => "ymir::config::validation",
        }))
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        None
    }
}

/// Error type that chains main errors with include errors.
///
/// Allows miette's Report formatting to have main + include errors all in one.
#[derive(Debug)]
pub struct ConfigIncludeError {
    pub main: ConfigError,
    pub includes: Vec<ConfigError>,
}

impl<T, E> ConfigParseResult<T, E> {
    pub fn from_err(err: E) -> Self {
        Self {
            config: Err(err),
            includes: Vec::new(),
        }
    }

    pub fn map_config_res<U, V>(
        self,
        f: impl FnOnce(Result<T, E>) -> Result<U, V>,
    ) -> ConfigParseResult<U, V> {
        ConfigParseResult {
            config: f(self.config),
            includes: self.includes,
        }
    }
}

impl fmt::Display for ConfigIncludeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.main, f)
    }
}

impl Error for ConfigIncludeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.main.source()
    }
}

impl Diagnostic for ConfigIncludeError {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.main.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.main.severity()
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.main.help()
    }

    fn url<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.main.url()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.main.source_code()
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        self.main.labels()
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        self.main.diagnostic_source()
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        let main_related = self.main.related();
        let includes_iter = self.includes.iter().map(|err| err as &'a dyn Diagnostic);

        let iter: Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a> = match main_related {
            Some(main) => Box::new(main.chain(includes_iter)),
            None => Box::new(includes_iter),
        };

        Some(iter)
    }
}