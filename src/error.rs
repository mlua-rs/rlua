use std::fmt;
use std::result::Result;
use std::error::Error;
use std::ffi::NulError;
use std::cell::{BorrowError, BorrowMutError};
use std::str::Utf8Error;

#[derive(Debug)]
pub struct LuaExternalError(pub Box<Error + Send>);

impl fmt::Display for LuaExternalError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.0.fmt(fmt)
    }
}

impl Error for LuaExternalError {
    fn description(&self) -> &str {
        self.0.description()
    }

    fn cause(&self) -> Option<&Error> {
        self.0.cause()
    }
}

error_chain! {
    types {
        LuaError, LuaErrorKind, LuaResultExt, LuaResult;
    }

    errors {
        ScriptError(err: String) {
            display("Error executing lua script {}", err)
        }
        CallbackError(err: String) {
            display("Error during lua callback {}", err)
        }
        IncompleteStatement(err: String) {
            display("Incomplete lua statement {}", err)
        }
    }

    foreign_links {
        ExternalError(LuaExternalError);
        Utf8Error(Utf8Error);
        NulError(NulError);
        BorrowError(BorrowError);
        BorrowMutError(BorrowMutError);
    }
}

/// Helper trait to convert external error types to a `LuaExternalError`
pub trait LuaExternalResult<T> {
    fn to_lua_err(self) -> LuaResult<T>;
}

impl<T, E> LuaExternalResult<T> for Result<T, E>
where
    E: 'static + Error + Send,
{
    fn to_lua_err(self) -> LuaResult<T> {
        self.map_err(|e| LuaExternalError(Box::new(e)).into())
    }
}
