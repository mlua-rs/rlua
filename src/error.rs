use std::fmt;
use std::sync::Arc;
use std::error::Error as StdError;
use std::result::Result as StdResult;

#[derive(Debug, Clone)]
pub enum Error {
    /// Lua syntax error, aka `LUA_ERRSYNTAX` that is NOT an incomplete statement.
    SyntaxError(String),
    /// Lua syntax error that IS an incomplete statement.  Useful for implementing a REPL.
    IncompleteStatement(String),
    /// Lua runtime error, aka `LUA_ERRRUN`.
    RuntimeError(String),
    /// Lua error from inside an error handler, aka `LUA_ERRERR`.
    ErrorError(String),
    /// A generic Rust -> Lua conversion error.
    ToLuaConversionError(String),
    /// A generic Lua -> Rust conversion error.
    FromLuaConversionError(String),
    /// A `Thread` was resumed and the coroutine was no longer active.
    CoroutineInactive,
    /// An `AnyUserData` is not the expected type in a borrow.
    UserDataTypeMismatch,
    /// An `AnyUserData` immutable borrow failed because it is already borrowed mutably.
    UserDataBorrowError,
    /// An `AnyUserData` mutable borrow failed because it is already borrowed.
    UserDataBorrowMutError,
    /// Lua error that originated as a Error in a callback.  The first field is the lua error as
    /// a string, the second field is the Arc holding the original Error.
    CallbackError(String, Arc<Error>),
    /// Any custom external error type, mostly useful for returning external error types from
    /// callbacks.
    ExternalError(Arc<StdError + Send + Sync>),
}

pub type Result<T> = StdResult<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::SyntaxError(ref msg) => write!(fmt, "Lua syntax error: {}", msg),
            Error::IncompleteStatement(ref msg) => {
                write!(fmt, "Lua syntax error (incomplete statement): {}", msg)
            }
            Error::RuntimeError(ref msg) => write!(fmt, "Lua runtime error: {}", msg),
            Error::ErrorError(ref msg) => write!(fmt, "Lua error in error handler: {}", msg),
            Error::ToLuaConversionError(ref msg) => {
                write!(fmt, "Error converting rust type to lua: {}", msg)
            }
            Error::FromLuaConversionError(ref msg) => {
                write!(fmt, "Error converting lua type to rust: {}", msg)
            }
            Error::CoroutineInactive => write!(fmt, "Cannot resume inactive coroutine"),
            Error::UserDataTypeMismatch => write!(fmt, "Userdata not expected type"),
            Error::UserDataBorrowError => write!(fmt, "Userdata already mutably borrowed"),
            Error::UserDataBorrowMutError => write!(fmt, "Userdata already borrowed"),
            Error::CallbackError(ref msg, _) => {
                write!(fmt, "Error during lua callback: {}", msg)
            }
            Error::ExternalError(ref err) => err.fmt(fmt),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::SyntaxError(_) => "lua syntax error",
            Error::IncompleteStatement(_) => "lua incomplete statement",
            Error::RuntimeError(_) => "lua runtime error",
            Error::ErrorError(_) => "lua error handling error",
            Error::ToLuaConversionError(_) => "conversion error to lua",
            Error::FromLuaConversionError(_) => "conversion error from lua",
            Error::CoroutineInactive => "lua coroutine inactive",
            Error::UserDataTypeMismatch => "lua userdata type mismatch",
            Error::UserDataBorrowError => "lua userdata already mutably borrowed",
            Error::UserDataBorrowMutError => "lua userdata already borrowed",
            Error::CallbackError(_, _) => "lua callback error",
            Error::ExternalError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::CallbackError(_, ref cause) => Some(cause.as_ref()),
            Error::ExternalError(ref err) => err.cause(),
            _ => None,
        }
    }
}

impl Error {
    pub fn external<T: 'static + StdError + Send + Sync>(err: T) -> Error {
        Error::ExternalError(Arc::new(err))
    }
}

pub trait ExternalError {
    fn to_lua_err(self) -> Error;
}

impl<E> ExternalError for E
where
    E: Into<Box<StdError + Send + Sync>>,
{
    fn to_lua_err(self) -> Error {
        #[derive(Debug)]
        struct WrapError(Box<StdError + Send + Sync>);

        impl fmt::Display for WrapError {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl StdError for WrapError {
            fn description(&self) -> &str {
                self.0.description()
            }
        }

        Error::external(WrapError(self.into()))
    }
}

pub trait ExternalResult<T> {
    fn to_lua_err(self) -> Result<T>;
}

impl<T, E> ExternalResult<T> for StdResult<T, E>
where
    E: ExternalError,
{
    fn to_lua_err(self) -> Result<T> {
        self.map_err(|e| e.to_lua_err())
    }
}
