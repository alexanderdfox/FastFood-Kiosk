# McKiosk OS (Modern bootloader 0.11)

## Build & Run

```bash
# Make sure you are on latest nightly
rustup default nightly
rustup component add rust-src llvm-tools-preview
rustup target add x86_64-unknown-none

cd mckiosk-os

# Important: force the unstable feature
cargo build -Z bindeps

# Run
cargo run -Z bindeps
```

Or more explicitly:

```bash
cargo build -Z bindeps
qemu-system-x86_64 -drive format=raw,file=target/debug/bios.img
```
