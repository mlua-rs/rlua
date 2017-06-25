use std::fmt;
use std::sync::Arc;
use std::result::Result;
use std::error::Error;
use std::ffi::NulError;
use std::str::Utf8Error;

#[derive(Debug, Clone)]
pub enum LuaSyntaxError {
    /// A generic syntax error
    Syntax(String),
    /// A syntax error due to an incomplete statement, useful for implementing a
    /// Lua REPL.
    IncompleteStatement(String),
}

#[derive(Debug, Clone)]
pub enum LuaConversionError {
    /// A generic Rust -> Lua type conversion error.
    ToLua(String),
    /// A generic Lua -> Rust type conversion error.
    FromLua(String),
    /// A Lua string was not valid utf8 on conversion to a rust String.
    Utf8Error(Utf8Error),
    /// A rust String contained a NUL character, and thus was not convertible to
    /// a Lua string.
    NulError(NulError),
}

#[derive(Debug, Clone)]
pub enum LuaUserDataError {
    /// A `LuaUserData` borrow failed because the expected type was not the
    /// contained type.
    TypeMismatch,
    /// A `LuaUserData` immutable borrow failed because it was already borrowed
    /// mutably.
    BorrowError,
    /// A `LuaUserData` mutable borrow failed because it was already borrowed.
    BorrowMutError,
}

#[derive(Debug, Clone)]
pub enum LuaError {
    /// Lua syntax error, aka LUA_ERRSYNTAX.
    SyntaxError(LuaSyntaxError),
    /// Lua runtime error, aka LUA_ERRRUN.
    RuntimeError(String),
    /// Lua error from inside an error handler, aka LUA_ERRERR.
    ErrorError(String),
    /// An error resulting from a Lua <-> Rust type conversion
    ConversionError(LuaConversionError),
    /// Insufficient Lua stack space.
    StackOverflow,
    /// A `LuaThread` was resumed and the coroutine was no longer active.
    CoroutineInactive,
    /// A `LuaUserData` borrow of the internal value failed.
    UserDataError(LuaUserDataError),
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
            &LuaError::SyntaxError(LuaSyntaxError::Syntax(ref msg)) => {
                write!(fmt, "Lua syntax error: {}", msg)
            }
            &LuaError::SyntaxError(LuaSyntaxError::IncompleteStatement(ref msg)) => {
                write!(fmt, "Lua syntax error: {}", msg)
            }

            &LuaError::RuntimeError(ref msg) => write!(fmt, "Lua runtime error: {}", msg),
            &LuaError::ErrorError(ref msg) => write!(fmt, "Lua error in error handler: {}", msg),

            &LuaError::ConversionError(LuaConversionError::ToLua(ref msg)) => {
                write!(fmt, "Error converting rust type to lua: {}", msg)
            }
            &LuaError::ConversionError(LuaConversionError::FromLua(ref msg)) => {
                write!(fmt, "Error converting lua type to rust: {}", msg)
            }
            &LuaError::ConversionError(LuaConversionError::Utf8Error(ref msg)) => {
                write!(fmt, "Error converting lua string to rust: {}", msg)
            }
            &LuaError::ConversionError(LuaConversionError::NulError(ref msg)) => {
                write!(fmt, "Error converting rust string to lua: {}", msg)
            }

            &LuaError::StackOverflow => write!(fmt, "Lua out of stack space"),
            &LuaError::CoroutineInactive => write!(fmt, "Cannot resume inactive coroutine"),

            &LuaError::UserDataError(LuaUserDataError::TypeMismatch) => {
                write!(fmt, "Userdata not expected type")
            }
            &LuaError::UserDataError(LuaUserDataError::BorrowError) => {
                write!(fmt, "Userdata already mutably borrowed")
            }
            &LuaError::UserDataError(LuaUserDataError::BorrowMutError) => {
                write!(fmt, "Userdata already borrowed")
            }

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
            &LuaError::SyntaxError(LuaSyntaxError::Syntax(_)) => "lua syntax error",
            &LuaError::SyntaxError(LuaSyntaxError::IncompleteStatement(_)) => {
                "lua incomplete statement"
            }

            &LuaError::RuntimeError(_) => "lua runtime error",
            &LuaError::ErrorError(_) => "lua error handling error",

            &LuaError::ConversionError(LuaConversionError::ToLua(_)) => "conversion error to lua",
            &LuaError::ConversionError(LuaConversionError::FromLua(_)) => {
                "conversion error from lua"
            }
            &LuaError::ConversionError(LuaConversionError::Utf8Error(_)) => {
                "lua string utf8 conversion error"
            }
            &LuaError::ConversionError(LuaConversionError::NulError(_)) => "string contains null",

            &LuaError::StackOverflow => "lua stack overflow",
            &LuaError::CoroutineInactive => "lua coroutine inactive",

            &LuaError::UserDataError(LuaUserDataError::TypeMismatch) => {
                "lua userdata type mismatch"
            }
            &LuaError::UserDataError(LuaUserDataError::BorrowError) => {
                "lua userdata already borrowed"
            }
            &LuaError::UserDataError(LuaUserDataError::BorrowMutError) => {
                "lua userdata already mutably borrowed"
            }

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

impl From<LuaSyntaxError> for LuaError {
    fn from(err: LuaSyntaxError) -> LuaError {
        LuaError::SyntaxError(err)
    }
}

impl From<LuaConversionError> for LuaError {
    fn from(err: LuaConversionError) -> LuaError {
        LuaError::ConversionError(err)
    }
}

impl From<LuaUserDataError> for LuaError {
    fn from(err: LuaUserDataError) -> LuaError {
        LuaError::UserDataError(err)
    }
}
