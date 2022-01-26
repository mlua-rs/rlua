# rlua release checklist

* Basic README.md check
* Update CHANGELOG.md with important changes since last release
* For a maintenance release:
  * Check if there are any bugfixes on master which should be included
* Update version number in Cargo.toml
* Check other fields in Cargo.toml are sensible
* Check that CI is passing
* Tag the commit for the release
* Run `cargo publish --dry-run` -sys crates if updated.
* Run `cargo publish --dry-run` on main crate.
* Run `cargo publish` on -sys crates if updated.
* Run `cargo publish`
* Check that the version from crates.io looks good
* Update version number on branch to (next version)-alpha.
