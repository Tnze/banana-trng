# Banana Random Number Generator Firmware

## Develop

```bash
rustup toolchain add thumbv7m-none-eabi
cargo install probe-rs-tools
cargo install cargo-binutils
rustup component add llvm-tools
```

## Debug

```bash
openocd -f interface/stlink.cfg -f target/stm32f1x.cfg
```

or

```bash
cargo run
```

## Release

```bash
cargo build --release
cargo objcopy --release -- -O binary app.bin
```
