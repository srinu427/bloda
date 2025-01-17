# BLOcked Directory Archive


An archival tool to make handling directories with huge number of small files in Network file systems better.

## Features

- Reduce number of files to be uploaded/copied to Network FS by archiving them to 2 files
- Reduce the storage used without losing the ability to get individual files from storage
- Designed with source code and coverage reports as a focus, but can be used with any directories with large number of small files
- Multiplatform support - The tool is compatible with Linux, macOS and Windows


## Download

Available releases can be downloaded for your platform of choice on the [Releases](https://github.com/zaszi/rust-template/releases) page. These are merely provided as an example on how the asset uploading works, and aren't actually useful by themselves beyond what a `hello world` program can provide.

## Usage

[CLI](bloda-cli/README.md)

[Python Library](bloda-pyo3/README.md)

## Building

If desired, you can build s3-seek-archive yourself. You will need a working `Rust` and `Cargo` setup. [Rustup](https://rustup.rs/) is the simplest way to set this up on either Windows, Mac or Linux.

Once the prerequisites have been installed, compilation on your native platform is as simple as running the following in a terminal:

```
cd rust-compressor
cargo build --release
```

## WebAssembly

No web assembly support since we need file IO

## Contribution

Found a problem or have a suggestion? Feel free to open an issue.

## License

idk