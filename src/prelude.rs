//! Re-exports most types with an extra `Lua*` prefix to prevent name clashes.

pub use crate::{
    AnyUserData as LuaAnyUserData, Chunk as LuaChunk, Context as LuaContext, Debug as LuaDebug,
    DebugNames as LuaDebugNames, DebugSource as LuaDebugSource, DebugStack as LuaDebugStack,
    Error as LuaError, ExternalError as LuaExternalError, ExternalResult as LuaExternalResult,
    Fallible as LuaFallible, FromLua, FromLuaMulti, Function as LuaFunction,
    HookTriggers as LuaHookTriggers, Integer as LuaInteger, LightUserData as LuaLightUserData, Lua,
    MetaMethod as LuaMetaMethod, MultiValue as LuaMultiValue, Nil as LuaNil, Number as LuaNumber,
    RegistryKey as LuaRegistryKey, Result as LuaResult, Scope as LuaScope, String as LuaString,
    Table as LuaTable, TablePairs as LuaTablePairs, TableSequence as LuaTableSequence,
    Thread as LuaThread, ThreadStatus as LuaThreadStatus, ToLua, ToLuaMulti,
    UserData as LuaUserData, UserDataMethods as LuaUserDataMethods, Value as LuaValue,
};
