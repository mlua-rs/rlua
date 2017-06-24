use std::fmt;
use std::sync::Arc;
use std::result::Result;
use std::error::Error;
use std::ffi::NulError;
use std::str::Utf8Error;

#[derive(Debug, Clone)]
pub enum LuaError {
    ScriptError(String),
    CallbackError(String, Arc<LuaError>),
    IncompleteStatement(String),
    CoroutineInactive,
    StackOverflow,
    UserDataBorrowError,
    UserDataBorrowMutError,
    Utf8Error(Utf8Error),
    NulError(NulError),
    ConversionError(String),
    ExternalError(Arc<Error + Send + Sync>),
}

pub type LuaResult<T> = Result<T, LuaError>;

impl fmt::Display for LuaError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            &LuaError::ScriptError(ref msg) => write!(fmt, "Error executing lua script: {}", msg),
            &LuaError::CallbackError(ref msg, _) => {
                write!(fmt, "Error during lua callback: {}", msg)
            }
            &LuaError::ExternalError(ref err) => err.fmt(fmt),
            &LuaError::IncompleteStatement(ref msg) => {
                write!(fmt, "Incomplete lua statement: {}", msg)
            }
            &LuaError::CoroutineInactive => write!(fmt, "Cannot resume inactive coroutine"),
            &LuaError::ConversionError(ref msg) => {
                write!(fmt, "Error converting lua type: {}", msg)
            }
            &LuaError::StackOverflow => write!(fmt, "Lua stack overflow"),
            &LuaError::UserDataBorrowError => write!(fmt, "Userdata already mutably borrowed"),
            &LuaError::UserDataBorrowMutError => write!(fmt, "Userdata already borrowed"),
            &LuaError::Utf8Error(ref err) => write!(fmt, "Lua string utf8 error: {}", err),
            &LuaError::NulError(ref err) => {
                write!(fmt, "String passed to lua contains null: {}", err)
            }
        }
    }
}

impl Error for LuaError {
    fn description(&self) -> &str {
        match self {
            &LuaError::ScriptError(_) => "lua script error",
            &LuaError::CallbackError(_, _) => "lua callback error",
            &LuaError::ExternalError(ref err) => err.description(),
            &LuaError::IncompleteStatement(_) => "lua incomplete statement",
            &LuaError::CoroutineInactive => "lua coroutine inactive",
            &LuaError::ConversionError(_) => "lua conversion error",
            &LuaError::StackOverflow => "lua stack overflow",
            &LuaError::UserDataBorrowError => "lua userdata already mutably borrowed",
            &LuaError::UserDataBorrowMutError => "lua userdata already borrowed",
            &LuaError::Utf8Error(_) => "lua string utf8 conversion error",
            &LuaError::NulError(_) => "string null error",
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
