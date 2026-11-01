#!/usr/bin/env sh
set -eu

TARGET="x86_64-unknown-linux-musl"
OUTPUT_DIR="dist"
OUTPUT="$OUTPUT_DIR/tunnel-helper-linux-x86_64"
ARCHIVE="$OUTPUT_DIR/tunnel-helper-linux-x86_64.tar.gz"
OUTPUT_NAME="$(basename "$OUTPUT")"
ARCHIVE_NAME="$(basename "$ARCHIVE")"

if ! command -v musl-gcc >/dev/null 2>&1; then
  echo "musl-gcc is required. On Ubuntu/Debian: sudo apt install musl-tools" >&2
  exit 1
fi

rustup target add "$TARGET"
cargo build --release --target "$TARGET"
mkdir -p "$OUTPUT_DIR"
install -m 755 "target/$TARGET/release/tunnel-helper" "$OUTPUT"
tar -czf "$ARCHIVE" \
  --transform='s|tunnel-helper-linux-x86_64|tunnel-helper|' \
  -C "$OUTPUT_DIR" "$OUTPUT_NAME" -C ../../web help
(
  cd "$OUTPUT_DIR"
  sha256sum "$OUTPUT_NAME" > "$OUTPUT_NAME.sha256"
  sha256sum "$ARCHIVE_NAME" > "$ARCHIVE_NAME.sha256"
)

echo "Portable binary: $OUTPUT"
echo "Release archive: $ARCHIVE"
file "$OUTPUT"
