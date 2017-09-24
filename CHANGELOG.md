## [0.9.2]
- Bugfix, don't load the "base" library into the "base" global variable
  @jonas-schievink
- Additional documentation work, a link fix for `Variadic` docs, new crate
  documentation @jonas-schievink
- Metatable access on `Table`
- `gcc` crate warning fix for 0.3.52 and up
- Bugfix for `Table::raw_get`, now actually calls raw_get and is sound.

## [0.9.1]
- Add travis badge

## [0.9.0]
- Huge API change, removed the `Lua` prefix on all types, changes to callback
  signature that remove the need for manual wrapping and unwrapping in most
  cases.
- Tons of soundness bugfixes, very few soundness problems remain.
- Tons of documentation and bugifx work @jonas-schievink

## [0.8.0]
- Major API change, out of stack space is no longer an Err, you should not be
  able to run out of stack space by using this API, except through bugs.
- Simplification of error types

## [0.7.0]
- API change to remove dependency on `error_chain`, major changes to error
  handling strategy to allow Lua to catch and rethrow rust errors sanely.
