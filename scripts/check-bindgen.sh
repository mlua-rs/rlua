#!/bin/bash
set -ex
 
for crate in crates/rlua-lua??-sys ; do
    # The bindings are not consistently ordered on different machines, which
    # seems to be to do with the system headers, so we compare the sorted
    # output.
    HASH_ORIG=$(git show HEAD:${crate}/src/bindings.rs | sort | sha256sum)
    HASH_NEW=$(sort < ${crate}/src/bindings.rs | sha256sum)
    if [ "$HASH_ORIG" != "$HASH_NEW" ] ; then
        echo "Error, ${crate}/src/bindings.rs differs."
        exit 1
    fi
done
