use std::marker::PhantomData;

use ffi;
use error::Result;
use util::*;
use types::{Integer, LuaRef};
use lua::{FromLua, ToLua};

/// Handle to an internal Lua table.
#[derive(Clone, Debug)]
pub struct Table<'lua>(pub(crate) LuaRef<'lua>);

impl<'lua> Table<'lua> {
    /// Sets a key-value pair in the table.
    ///
    /// If the value is `nil`, this will effectively remove the pair.
    ///
    /// This might invoke the `__newindex` metamethod. Use the [`raw_set`] method if that is not
    /// desired.
    ///
    /// # Examples
    ///
    /// Export a value as a global to make it usable from Lua:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let globals = lua.globals();
    ///
    /// globals.set("assertions", cfg!(debug_assertions))?;
    ///
    /// lua.exec::<()>(r#"
    ///     if assertions == true then
    ///         -- ...
    ///     elseif assertions == false then
    ///         -- ...
    ///     else
    ///         error("assertions neither on nor off?")
    ///     end
    /// "#, None)?;
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`raw_set`]: #method.raw_set
    pub fn set<K: ToLua<'lua>, V: ToLua<'lua>>(&self, key: K, value: V) -> Result<()> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 6);
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?);
                lua.push_value(lua.state, value.to_lua(lua)?);
                protect_lua_call(lua.state, 3, 0, |state| {
                    ffi::lua_settable(state, -3);
                })
            })
        }
    }

    /// Gets the value associated to `key` from the table.
    ///
    /// If no value is associated to `key`, returns the `nil` value.
    ///
    /// This might invoke the `__index` metamethod. Use the [`raw_get`] method if that is not
    /// desired.
    ///
    /// # Examples
    ///
    /// Query the version of the Lua interpreter:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let globals = lua.globals();
    ///
    /// let version: String = globals.get("_VERSION")?;
    /// println!("Lua version: {}", version);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`raw_get`]: #method.raw_get
    pub fn get<K: ToLua<'lua>, V: FromLua<'lua>>(&self, key: K) -> Result<V> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 5);
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?);
                protect_lua_call(lua.state, 2, 1, |state| ffi::lua_gettable(state, -2))?;
                V::from_lua(lua.pop_value(lua.state), lua)
            })
        }
    }

    /// Checks whether the table contains a non-nil value for `key`.
    pub fn contains_key<K: ToLua<'lua>>(&self, key: K) -> Result<bool> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 5);
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?);
                protect_lua_call(lua.state, 2, 1, |state| ffi::lua_gettable(state, -2))?;
                let has = ffi::lua_isnil(lua.state, -1) == 0;
                ffi::lua_pop(lua.state, 1);
                Ok(has)
            })
        }
    }

    /// Sets a key-value pair without invoking metamethods.
    pub fn raw_set<K: ToLua<'lua>, V: ToLua<'lua>>(&self, key: K, value: V) -> Result<()> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 3);
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?);
                lua.push_value(lua.state, value.to_lua(lua)?);
                protect_lua_call(lua.state, 3, 0, |state| {
                    ffi::lua_rawset(state, -3);
                })?;
                Ok(())
            })
        }
    }

    /// Gets the value associated to `key` without invoking metamethods.
    pub fn raw_get<K: ToLua<'lua>, V: FromLua<'lua>>(&self, key: K) -> Result<V> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 2);
                lua.push_ref(lua.state, &self.0);
                lua.push_value(lua.state, key.to_lua(lua)?);
                ffi::lua_rawget(lua.state, -2);
                let res = V::from_lua(lua.pop_value(lua.state), lua)?;
                ffi::lua_pop(lua.state, 1);
                Ok(res)
            })
        }
    }

    /// Returns the result of the Lua `#` operator.
    ///
    /// This might invoke the `__len` metamethod. Use the [`raw_len`] method if that is not desired.
    ///
    /// [`raw_len`]: #method.raw_len
    pub fn len(&self) -> Result<Integer> {
        let lua = self.0.lua;
        unsafe {
            stack_err_guard(lua.state, 0, || {
                check_stack(lua.state, 4);
                lua.push_ref(lua.state, &self.0);
                protect_lua_call(lua.state, 1, 0, |state| ffi::luaL_len(state, -1))
            })
        }
    }

    /// Returns the result of the Lua `#` operator, without invoking the `__len` metamethod.
    pub fn raw_len(&self) -> Integer {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1);
                lua.push_ref(lua.state, &self.0);
                let len = ffi::lua_rawlen(lua.state, -1);
                ffi::lua_pop(lua.state, 1);
                len as Integer
            })
        }
    }

    /// Returns a reference to the metatable of this table, or `None` if no metatable is set.
    ///
    /// Unlike the `getmetatable` Lua function, this method ignores the `__metatable` field.
    pub fn get_metatable(&self) -> Option<Table<'lua>> {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, || {
                check_stack(lua.state, 1);
                lua.push_ref(lua.state, &self.0);
                if ffi::lua_getmetatable(lua.state, -1) == 0 {
                    ffi::lua_pop(lua.state, 1);
                    None
                } else {
                    let table = Table(lua.pop_ref(lua.state));
                    ffi::lua_pop(lua.state, 1);
                    Some(table)
                }
            })
        }
    }

    /// Sets or removes the metatable of this table.
    ///
    /// If `metatable` is `None`, the metatable is removed (if no metatable is set, this does
    /// nothing).
    pub fn set_metatable(&self, metatable: Option<Table<'lua>>) {
        let lua = self.0.lua;
        unsafe {
            stack_guard(lua.state, 0, move || {
                check_stack(lua.state, 1);
                lua.push_ref(lua.state, &self.0);
                if let Some(metatable) = metatable {
                    lua.push_ref(lua.state, &metatable.0);
                } else {
                    ffi::lua_pushnil(lua.state);
                }
                ffi::lua_setmetatable(lua.state, -2);
                ffi::lua_pop(lua.state, 1);
            })
        }
    }

    /// Consume this table and return an iterator over the pairs of the table.
    ///
    /// This works like the Lua `pairs` function, but does not invoke the `__pairs` metamethod.
    ///
    /// The pairs are wrapped in a [`Result`], since they are lazily converted to `K` and `V` types.
    ///
    /// # Note
    ///
    /// While this method consumes the `Table` object, it can not prevent code from mutating the
    /// table while the iteration is in progress. Refer to the [Lua manual] for information about
    /// the consequences of such mutation.
    ///
    /// # Examples
    ///
    /// Iterate over all globals:
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result, Value};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let globals = lua.globals();
    ///
    /// for pair in globals.pairs::<Value, Value>() {
    ///     let (key, value) = pair?;
    /// #   let _ = (key, value);   // used
    ///     // ...
    /// }
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`Result`]: type.Result.html
    /// [Lua manual]: http://www.lua.org/manual/5.3/manual.html#pdf-next
    pub fn pairs<K: FromLua<'lua>, V: FromLua<'lua>>(self) -> TablePairs<'lua, K, V> {
        let next_key = Some(LuaRef {
            lua: self.0.lua,
            registry_id: ffi::LUA_REFNIL,
        });

        TablePairs {
            table: self.0,
            next_key,
            _phantom: PhantomData,
        }
    }

    /// Consume this table and return an iterator over all values in the sequence part of the table.
    ///
    /// The iterator will yield all values `t[1]`, `t[2]`, and so on, until a `nil` value is
    /// encountered. This mirrors the behaviour of Lua's `ipairs` function and will invoke the
    /// `__index` metamethod according to the usual rules. However, the deprecated `__ipairs`
    /// metatable will not be called.
    ///
    /// Just like [`pairs`], the values are wrapped in a [`Result`].
    ///
    /// # Note
    ///
    /// While this method consumes the `Table` object, it can not prevent code from mutating the
    /// table while the iteration is in progress. Refer to the [Lua manual] for information about
    /// the consequences of such mutation.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate rlua;
    /// # use rlua::{Lua, Result, Table};
    /// # fn try_main() -> Result<()> {
    /// let lua = Lua::new();
    /// let my_table: Table = lua.eval("{ [1] = 4, [2] = 5, [4] = 7, key = 2 }", None)?;
    ///
    /// let expected = [4, 5];
    /// for (&expected, got) in expected.iter().zip(my_table.sequence_values::<u32>()) {
    ///     assert_eq!(expected, got?);
    /// }
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`pairs`]: #method.pairs
    /// [`Result`]: type.Result.html
    /// [Lua manual]: http://www.lua.org/manual/5.3/manual.html#pdf-next
    pub fn sequence_values<V: FromLua<'lua>>(self) -> TableSequence<'lua, V> {
        TableSequence {
            table: self.0,
            index: Some(1),
            _phantom: PhantomData,
        }
    }
}

/// An iterator over the pairs of a Lua table.
///
/// This struct is created by the [`Table::pairs`] method.
///
/// [`Table::pairs`]: struct.Table.html#method.pairs
pub struct TablePairs<'lua, K, V> {
    table: LuaRef<'lua>,
    next_key: Option<LuaRef<'lua>>,
    _phantom: PhantomData<(K, V)>,
}

impl<'lua, K, V> Iterator for TablePairs<'lua, K, V>
where
    K: FromLua<'lua>,
    V: FromLua<'lua>,
{
    type Item = Result<(K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next_key) = self.next_key.take() {
            let lua = self.table.lua;

            unsafe {
                stack_guard(lua.state, 0, || {
                    check_stack(lua.state, 5);

                    lua.push_ref(lua.state, &self.table);
                    lua.push_ref(lua.state, &next_key);

                    match protect_lua_call(lua.state, 2, ffi::LUA_MULTRET, |state| {
                        if ffi::lua_next(state, -2) == 0 {
                            0
                        } else {
                            1
                        }
                    }) {
                        Ok(0) => {
                            ffi::lua_pop(lua.state, 1);
                            None
                        }
                        Ok(_) => {
                            ffi::lua_pushvalue(lua.state, -2);
                            let key = lua.pop_value(lua.state);
                            let value = lua.pop_value(lua.state);
                            self.next_key = Some(lua.pop_ref(lua.state));
                            ffi::lua_pop(lua.state, 1);

                            Some((|| {
                                let key = K::from_lua(key, lua)?;
                                let value = V::from_lua(value, lua)?;
                                Ok((key, value))
                            })())
                        }
                        Err(e) => Some(Err(e)),
                    }
                })
            }
        } else {
            None
        }
    }
}

/// An iterator over the sequence part of a Lua table.
///
/// This struct is created by the [`Table::sequence_values`] method.
///
/// [`Table::sequence_values`]: struct.Table.html#method.sequence_values
pub struct TableSequence<'lua, V> {
    table: LuaRef<'lua>,
    index: Option<Integer>,
    _phantom: PhantomData<V>,
}

impl<'lua, V> Iterator for TableSequence<'lua, V>
where
    V: FromLua<'lua>,
{
    type Item = Result<V>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(index) = self.index.take() {
            let lua = self.table.lua;

            unsafe {
                stack_guard(lua.state, 0, || {
                    check_stack(lua.state, 4);

                    lua.push_ref(lua.state, &self.table);
                    match protect_lua_call(lua.state, 1, 1, |state| ffi::lua_geti(state, -1, index))
                    {
                        Ok(ffi::LUA_TNIL) => {
                            ffi::lua_pop(lua.state, 1);
                            None
                        }
                        Ok(_) => {
                            let value = lua.pop_value(lua.state);
                            self.index = Some(index + 1);
                            Some(V::from_lua(value, lua))
                        }
                        Err(err) => Some(Err(err)),
                    }
                })
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Table;
    use error::Result;
    use lua::{Lua, Nil, Value};

    #[test]
    fn test_set_get() {
        let lua = Lua::new();
        let globals = lua.globals();
        globals.set("foo", "bar").unwrap();
        globals.set("baz", "baf").unwrap();
        assert_eq!(globals.get::<_, String>("foo").unwrap(), "bar");
        assert_eq!(globals.get::<_, String>("baz").unwrap(), "baf");
    }

    #[test]
    fn test_table() {
        let lua = Lua::new();
        let globals = lua.globals();

        globals.set("table", lua.create_table().unwrap()).unwrap();
        let table1: Table = globals.get("table").unwrap();
        let table2: Table = globals.get("table").unwrap();

        table1.set("foo", "bar").unwrap();
        table2.set("baz", "baf").unwrap();

        assert_eq!(table2.get::<_, String>("foo").unwrap(), "bar");
        assert_eq!(table1.get::<_, String>("baz").unwrap(), "baf");

        lua.exec::<()>(
            r#"
                table1 = {1, 2, 3, 4, 5}
                table2 = {}
                table3 = {1, 2, nil, 4, 5}
            "#,
            None,
        ).unwrap();

        let table1 = globals.get::<_, Table>("table1").unwrap();
        let table2 = globals.get::<_, Table>("table2").unwrap();
        let table3 = globals.get::<_, Table>("table3").unwrap();

        assert_eq!(table1.len().unwrap(), 5);
        assert_eq!(
            table1
                .clone()
                .pairs()
                .collect::<Result<Vec<(i64, i64)>>>()
                .unwrap(),
            vec![(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]
        );
        assert_eq!(
            table1
                .clone()
                .sequence_values()
                .collect::<Result<Vec<i64>>>()
                .unwrap(),
            vec![1, 2, 3, 4, 5]
        );

        assert_eq!(table2.len().unwrap(), 0);
        assert_eq!(
            table2
                .clone()
                .pairs()
                .collect::<Result<Vec<(i64, i64)>>>()
                .unwrap(),
            vec![]
        );
        assert_eq!(
            table2
                .sequence_values()
                .collect::<Result<Vec<i64>>>()
                .unwrap(),
            vec![]
        );

        // sequence_values should only iterate until the first border
        assert_eq!(
            table3
                .sequence_values()
                .collect::<Result<Vec<i64>>>()
                .unwrap(),
            vec![1, 2]
        );

        globals
            .set(
                "table4",
                lua.create_sequence_from(vec![1, 2, 3, 4, 5]).unwrap(),
            )
            .unwrap();
        let table4 = globals.get::<_, Table>("table4").unwrap();
        assert_eq!(
            table4.pairs().collect::<Result<Vec<(i64, i64)>>>().unwrap(),
            vec![(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]
        );
    }

    #[test]
    fn test_table_scope() {
        let lua = Lua::new();
        let globals = lua.globals();
        lua.exec::<()>(
            r#"
            touter = {
                tin = {1, 2, 3}
            }
        "#,
            None,
        ).unwrap();

        // Make sure that table gets do not borrow the table, but instead just borrow lua.
        let tin;
        {
            let touter = globals.get::<_, Table>("touter").unwrap();
            tin = touter.get::<_, Table>("tin").unwrap();
        }

        assert_eq!(tin.get::<_, i64>(1).unwrap(), 1);
        assert_eq!(tin.get::<_, i64>(2).unwrap(), 2);
        assert_eq!(tin.get::<_, i64>(3).unwrap(), 3);
    }

    #[test]
    fn test_metatable() {
        let lua = Lua::new();

        let table = lua.create_table().unwrap();
        let metatable = lua.create_table().unwrap();
        metatable
            .set(
                "__index",
                lua.create_function(|_, ()| Ok("index_value")).unwrap(),
            )
            .unwrap();
        table.set_metatable(Some(metatable));
        assert_eq!(table.get::<_, String>("any_key").unwrap(), "index_value");
        match table.raw_get::<_, Value>("any_key").unwrap() {
            Nil => {}
            _ => panic!(),
        }
        table.set_metatable(None);
        match table.get::<_, Value>("any_key").unwrap() {
            Nil => {}
            _ => panic!(),
        };
    }

    #[test]
    fn test_table_error() {
        let lua = Lua::new();
        let globals = lua.globals();
        lua.exec::<()>(
            r#"
            table = {}
            setmetatable(table, {
                __index = function()
                    error("lua error")
                end,
                __newindex = function()
                    error("lua error")
                end,
                __len = function()
                    error("lua error")
                end
            })
        "#,
            None,
        ).unwrap();

        let bad_table: Table = globals.get("table").unwrap();
        assert!(bad_table.set(1, 1).is_err());
        assert!(bad_table.get::<_, i32>(1).is_err());
        assert!(bad_table.len().is_err());
        assert!(bad_table.raw_set(1, 1).is_ok());
        assert!(bad_table.raw_get::<_, i32>(1).is_ok());
        assert_eq!(bad_table.raw_len(), 1);
    }
}
