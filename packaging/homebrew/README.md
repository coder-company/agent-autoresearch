# Homebrew Packaging

`autoresearch.rb.template` is the formula source for a Homebrew tap release. After the GitHub `Release` workflow publishes archives, replace `VERSION` and the `SHA256_*` placeholders with values from the uploaded `.sha256` files, then copy the rendered formula into the tap as `Formula/autoresearch.rb`.

The formula uses the same target-named archives consumed by `cargo-binstall`, so release asset names must stay in the form `autoresearch-v<VERSION>-<rust-target>.tar.gz`.
