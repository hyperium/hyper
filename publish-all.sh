#/bin/sh

# If the versions haven't changed, this'll just error, but we'll just keep chugging.
# That's fine. That said, just bump the version numbers when you'd like to publish.

cargo publish --manifest-path src/core;
cargo publish --manifest-path src/protocol;
cargo publish --manifest-path src/net;
cargo publish --manifest-path src/client;
cargo publish --manifest-path src/server;
cargo publish --manifest-path .;
