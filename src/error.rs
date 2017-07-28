use std::fmt;
use std::sync::Arc;
use std::error::Error as StdError;
use std::result::Result as StdResult;

/// Error type returned by rlua methods.
#[derive(Debug, Clone)]
pub enum Error {
    /// Lua syntax error, aka `LUA_ERRSYNTAX` that is NOT an incomplete statement.
    SyntaxError(String),
    /// Lua syntax error that IS an incomplete statement.  Useful for implementing a REPL.
    IncompleteStatement(String),
    /// Lua runtime error, aka `LUA_ERRRUN`.
    ///
    /// The Lua VM returns this error when a builtin operation is performed on incompatible types.
    /// Among other things, this includes invoking operators on wrong types (such as calling or
    /// indexing a `nil` value).
    RuntimeError(String),
    /// Lua error from inside an error handler, aka `LUA_ERRERR`.
    ///
    /// To prevent an infinite recursion when invoking an error handler, this error will be returned
    /// instead of invoking the error handler.
    ErrorError(String),
    /// A Rust value could not be converted to a Lua value.
    ToLuaConversionError(String),
    /// A Lua value could not be converted to the expected Rust type.
    FromLuaConversionError(String),
    /// [`Thread::resume`] was called on an inactive coroutine.
    ///
    /// A coroutine is inactive if its main function has returned or if an error has occured inside
    /// the coroutine.
    ///
    /// [`Thread::status`] can be used to check if the coroutine can be resumed without causing this
    /// error.
    ///
    /// [`Thread::resume`]: struct.Thread.html#method.resume
    /// [`Thread::status`]: struct.Thread.html#method.status
    CoroutineInactive,
    /// An [`AnyUserData`] is not the expected type in a borrow.
    ///
    /// This error can only happen when manually using [`AnyUserData`], or when implementing
    /// metamethods for binary operators. Refer to the documentation of [`UserDataMethods`] for
    /// details.
    ///
    /// [`AnyUserData`]: struct.AnyUserData.html
    /// [`UserDataMethods`]: struct.UserDataMethods.html
    UserDataTypeMismatch,
    /// An [`AnyUserData`] immutable borrow failed because it is already borrowed mutably.
    ///
    /// This error can occur when a method on a [`UserData`] type calls back into Lua, which then
    /// tries to call a method on the same [`UserData`] type. Consider restructuring your API to
    /// prevent these errors.
    ///
    /// [`AnyUserData`]: struct.AnyUserData.html
    /// [`UserData`]: trait.UserData.html
    UserDataBorrowError,
    /// An [`AnyUserData`] mutable borrow failed because it is already borrowed.
    ///
    /// This error can occur when a method on a [`UserData`] type calls back into Lua, which then
    /// tries to call a method on the same [`UserData`] type. Consider restructuring your API to
    /// prevent these errors.
    ///
    /// [`AnyUserData`]: struct.AnyUserData.html
    /// [`UserData`]: trait.UserData.html
    UserDataBorrowMutError,
    /// A Rust callback returned `Err`, raising the contained `Error` as a Lua error.
    ///
    /// The first field is the Lua traceback, the second field holds the original error.
    CallbackError(String, Arc<Error>),
    /// A custom error.
    ///
    /// This can be used for returning user-defined errors from callbacks.
    ///
    /// Returning `Err(ExternalError(...))` from a Rust callback will raise the error as a Lua
    /// error. The Rust code that originally invoked the Lua code then receives a `CallbackError`,
    /// from which the original error (and a stack traceback) can be recovered.
    ExternalError(Arc<StdError + Send + Sync>),
}

/// A specialized `Result` type used by rlua's API.
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
            Error::CallbackError(ref msg, _) => write!(fmt, "Error during lua callback: {}", msg),
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
