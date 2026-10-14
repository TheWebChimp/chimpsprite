use cs_commands::CmdError;
use cs_io::files::FileError;

/// Error de una herramienta; el mensaje lo lee el agente.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Command(#[from] CmdError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Io(#[from] cs_io::IoError),
    #[error("{0}")]
    Fs(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AgentError>;

/// Atajo para errores de validación.
pub(crate) fn invalid(message: impl Into<String>) -> AgentError {
    AgentError::Invalid(message.into())
}
