use std::fmt;
use std::sync::Arc;
use std::result::Result;
use std::error::Error;

#[derive(Debug, Clone)]
pub enum LuaError {
    /// Lua syntax error, aka `LUA_ERRSYNTAX` that is NOT an incomplete
    /// statement.
    SyntaxError(String),
    /// Lua syntax error that IS an incomplete statement.  Useful for
    /// implementing a REPL.
    IncompleteStatement(String),
    /// Lua runtime error, aka `LUA_ERRRUN`.
    RuntimeError(String),
    /// Lua error from inside an error handler, aka `LUA_ERRERR`.
    ErrorError(String),
    /// A generic Rust -> Lua conversion error.
    ToLuaConversionError(String),
    /// A generic Lua -> Rust conversion error.
    FromLuaConversionError(String),
    /// A `LuaThread` was resumed and the coroutine was no longer active.
    CoroutineInactive,
    /// A `LuaUserData` is not the expected type in a borrow.
    UserDataTypeMismatch,
    /// A `LuaUserData` immutable borrow failed because it is already borrowed
    /// mutably.
    UserDataBorrowError,
    /// A `LuaUserData` mutable borrow failed because it is already borrowed.
    UserDataBorrowMutError,
    /// Lua error that originated as a LuaError in a callback.  The first field
    /// is the lua error as a string, the second field is the Arc holding the
    /// original LuaError.
    CallbackError(String, Arc<LuaError>),
    /// Any custom external error type, mostly useful for returning external
    /// error types from callbacks.
    ExternalError(Arc<Error + Send + Sync>),
}

pub type LuaResult<T> = Result<T, LuaError>;

impl fmt::Display for LuaError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            &LuaError::SyntaxError(ref msg) => write!(fmt, "Lua syntax error: {}", msg),
            &LuaError::IncompleteStatement(ref msg) => {
                write!(fmt, "Lua syntax error (incomplete statement): {}", msg)
            }
            &LuaError::RuntimeError(ref msg) => write!(fmt, "Lua runtime error: {}", msg),
            &LuaError::ErrorError(ref msg) => write!(fmt, "Lua error in error handler: {}", msg),
            &LuaError::ToLuaConversionError(ref msg) => {
                write!(fmt, "Error converting rust type to lua: {}", msg)
            }
            &LuaError::FromLuaConversionError(ref msg) => {
                write!(fmt, "Error converting lua type to rust: {}", msg)
            }
            &LuaError::CoroutineInactive => write!(fmt, "Cannot resume inactive coroutine"),
            &LuaError::UserDataTypeMismatch => write!(fmt, "Userdata not expected type"),
            &LuaError::UserDataBorrowError => write!(fmt, "Userdata already mutably borrowed"),
            &LuaError::UserDataBorrowMutError => write!(fmt, "Userdata already borrowed"),
            &LuaError::CallbackError(ref msg, _) => {
                write!(fmt, "Error during lua callback: {}", msg)
            }
            &LuaError::ExternalError(ref err) => err.fmt(fmt),
        }
    }
}

impl Error for LuaError {
    fn description(&self) -> &str {
        match self {
            &LuaError::SyntaxError(_) => "lua syntax error",
            &LuaError::IncompleteStatement(_) => "lua incomplete statement",
            &LuaError::RuntimeError(_) => "lua runtime error",
            &LuaError::ErrorError(_) => "lua error handling error",
            &LuaError::ToLuaConversionError(_) => "conversion error to lua",
            &LuaError::FromLuaConversionError(_) => "conversion error from lua",
            &LuaError::CoroutineInactive => "lua coroutine inactive",
            &LuaError::UserDataTypeMismatch => "lua userdata type mismatch",
            &LuaError::UserDataBorrowError => "lua userdata already mutably borrowed",
            &LuaError::UserDataBorrowMutError => "lua userdata already borrowed",
            &LuaError::CallbackError(_, _) => "lua callback error",
            &LuaError::ExternalError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self {
            &LuaError::CallbackError(_, ref cause) => Some(cause.as_ref()),
            &LuaError::ExternalError(ref err) => err.cause(),
            _ => None,
        }
    }
}

impl LuaError {
    pub fn external<T: 'static + Error + Send + Sync>(err: T) -> LuaError {
        LuaError::ExternalError(Arc::new(err))
    }
}

pub trait LuaExternalError {
    fn to_lua_err(self) -> LuaError;
}

impl<E> LuaExternalError for E
where
    E: Into<Box<Error + Send + Sync>>,
{
    fn to_lua_err(self) -> LuaError {
        #[derive(Debug)]
        struct WrapError(Box<Error + Send + Sync>);

        impl fmt::Display for WrapError {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl Error for WrapError {
            fn description(&self) -> &str {
                self.0.description()
            }
        }

        LuaError::external(WrapError(self.into()))
    }
}

pub trait LuaExternalResult<T> {
    fn to_lua_err(self) -> LuaResult<T>;
}

impl<T, E> LuaExternalResult<T> for Result<T, E>
where
    E: LuaExternalError,
{
    fn to_lua_err(self) -> LuaResult<T> {
        self.map_err(|e| e.to_lua_err())
    }
}
