use std::fmt;
use std::sync::Arc;
use std::error::Error as StdError;
use std::result::Result as StdResult;

/// Error type returned by `rlua` methods.
#[derive(Debug, Clone)]
pub enum Error {
    /// Syntax error while parsing Lua source code.
    SyntaxError {
        /// The error message as returned by Lua.
        message: String,
        /// `true` if the error can likely be fixed by appending more input to the source code.
        ///
        /// This is useful for implementing REPLs as they can query the user for more input if this
        /// is set.
        incomplete_input: bool,
    },
    /// Lua runtime error, aka `LUA_ERRRUN`.
    ///
    /// The Lua VM returns this error when a builtin operation is performed on incompatible types.
    /// Among other things, this includes invoking operators on wrong types (such as calling or
    /// indexing a `nil` value).
    RuntimeError(String),
    /// Lua garbage collector error, aka `LUA_ERRGCMM`.
    ///
    /// The Lua VM returns this error when there is an error running a `__gc` metamethod.
    GarbageCollectorError(String),
    /// A callback has triggered Lua code that has called the same callback again.
    ///
    /// This is an error because `rlua` callbacks are FnMut and thus can only be mutably borrowed
    /// once.
    RecursiveCallbackError,
    /// Lua code has accessed a [`UserData`] value that was already garbage collected
    ///
    /// This can happen when a [`UserData`] has a custom `__gc` metamethod, this method resurrects
    /// the [`UserData`], and then the [`UserData`] is subsequently accessed.
    /// [`UserData`]: trait.UserData.html
    ExpiredUserData,
    /// A Rust value could not be converted to a Lua value.
    ToLuaConversionError {
        /// Name of the Rust type that could not be converted.
        from: &'static str,
        /// Name of the Lua type that could not be created.
        to: &'static str,
        /// A message indicating why the conversion failed in more detail.
        message: Option<String>,
    },
    /// A Lua value could not be converted to the expected Rust type.
    FromLuaConversionError {
        /// Name of the Lua type that could not be converted.
        from: &'static str,
        /// Name of the Rust type that could not be created.
        to: &'static str,
        /// A string containing more detailed error information.
        message: Option<String>,
    },
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
    CallbackError {
        /// Lua call stack backtrace.
        traceback: String,
        /// Original error returned by the Rust code.
        cause: Arc<Error>,
    },
    /// A custom error.
    ///
    /// This can be used for returning user-defined errors from callbacks.
    ///
    /// Returning `Err(ExternalError(...))` from a Rust callback will raise the error as a Lua
    /// error. The Rust code that originally invoked the Lua code then receives a `CallbackError`,
    /// from which the original error (and a stack traceback) can be recovered.
    ExternalError(Arc<StdError + Send + Sync>),
}

/// A specialized `Result` type used by `rlua`'s API.
pub type Result<T> = StdResult<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::SyntaxError { ref message, .. } => write!(fmt, "syntax error: {}", message),
            Error::RuntimeError(ref msg) => write!(fmt, "runtime error: {}", msg),
            Error::GarbageCollectorError(ref msg) => {
                write!(fmt, "garbage collector error: {}", msg)
            }
            Error::RecursiveCallbackError => write!(fmt, "callback called recursively"),
            Error::ExpiredUserData => write!(
                fmt,
                "access of userdata which has already been garbage collected"
            ),
            Error::ToLuaConversionError {
                from,
                to,
                ref message,
            } => {
                write!(fmt, "error converting {} to Lua {}", from, to)?;
                match *message {
                    None => Ok(()),
                    Some(ref message) => write!(fmt, " ({})", message),
                }
            }
            Error::FromLuaConversionError {
                from,
                to,
                ref message,
            } => {
                write!(fmt, "error converting Lua {} to {}", from, to)?;
                match *message {
                    None => Ok(()),
                    Some(ref message) => write!(fmt, " ({})", message),
                }
            }
            Error::CoroutineInactive => write!(fmt, "cannot resume inactive coroutine"),
            Error::UserDataTypeMismatch => write!(fmt, "userdata is not expected type"),
            Error::UserDataBorrowError => write!(fmt, "userdata already mutably borrowed"),
            Error::UserDataBorrowMutError => write!(fmt, "userdata already borrowed"),
            Error::CallbackError { ref traceback, .. } => {
                write!(fmt, "callback error: {}", traceback)
            }
            Error::ExternalError(ref err) => err.fmt(fmt),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::SyntaxError { .. } => "syntax error",
            Error::RuntimeError(_) => "runtime error",
            Error::GarbageCollectorError(_) => "garbage collector error",
            Error::RecursiveCallbackError => "callback called recursively",
            Error::ExpiredUserData => "access of userdata which has already been garbage collected",
            Error::ToLuaConversionError { .. } => "conversion error to lua",
            Error::FromLuaConversionError { .. } => "conversion error from lua",
            Error::CoroutineInactive => "attempt to resume inactive coroutine",
            Error::UserDataTypeMismatch => "userdata type mismatch",
            Error::UserDataBorrowError => "userdata already mutably borrowed",
            Error::UserDataBorrowMutError => "userdata already borrowed",
            Error::CallbackError { .. } => "callback error",
            Error::ExternalError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::CallbackError { ref cause, .. } => Some(cause.as_ref()),
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
        struct WrapError(Box<StdError + Send + Sync>);

        impl fmt::Debug for WrapError {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                fmt::Debug::fmt(&self.0, f)
            }
        }

        impl fmt::Display for WrapError {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl StdError for WrapError {
            fn description(&self) -> &str {
                self.0.description()
            }

            fn cause(&self) -> Option<&StdError> {
                self.0.cause()
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
