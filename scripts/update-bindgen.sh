#!/bin/bash

for crate in crates/rlua-lua??-sys ; do
    bindgen --size_t-is-usize ${crate}/wrapper_lua*.h -o ${crate}/src/bindings.rs -- -I ${crate}/lua-5.*/src/
done
