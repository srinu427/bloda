# BLOcked Directory Archive CLI


CLI for BLODA archival tool


## Usage

Bloda currently supports the commands compress and decompress for compressing a directory and decompressing an archive respectively

```
Usage: bloda <COMMAND>

Commands:
  compress    
  decompress  
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

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