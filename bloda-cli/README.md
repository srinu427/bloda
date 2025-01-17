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

Compress command options

```
./bloda compress --help
Usage: bloda compress [OPTIONS] --input-path <INPUT_PATH> --output-path <OUTPUT_PATH>

Options:
  -i, --input-path <INPUT_PATH>      Input directory name. If a file is provided, empty archive is generated
  -o, --output-path <OUTPUT_PATH>    Output file's name. Expected extention name is .bda
  -t, --thread-count <THREAD_COUNT>  Number of block to compress in parallel [default: 1]
  -c, --compression <COMPRESSION>    Compression to use. Defaults to ZSTD supported: LZMA, LZ4, ZSTD, NONE [default: ZSTD]
  -b, --block-size <BLOCK_SIZE>      Max size of file in bytes to be processed in memory instead of writing to temp file. Use 0 to reduce RAM usage [default: 67108864]
  -h, --help                         Print help
  -V, --version                      Print version
```

Decompress command options

```
./bloda decompress --help
Usage: bloda decompress [OPTIONS] --input-arc <INPUT_ARC> --output-dir <OUTPUT_DIR>

Options:
  -i, --input-arc <INPUT_ARC>        Input archive name. Expecting a .bda file
  -o, --output-dir <OUTPUT_DIR>      Output Dir name. Will be created if not present
  -t, --thread-count <THREAD_COUNT>  Number of block to compress in parallel [default: 1]
  -h, --help                         Print help
  -V, --version                      Print version
```

## Building

To build BLODA CLI, you will need a working `Rust` and `Cargo` setup. [Rustup](https://rustup.rs/) is the simplest way to set this up on either Windows, Mac or Linux.

You will need libsqlite3 installed during build time.
You will also optionally need liblzma and libzstd installed to use those compression formats in runtime.

Once the prerequisites have been installed, compilation on your native platform is as simple as running the following in a terminal:

```
git clone https://github.com/srinu427/bloda.git
cd bloda/bloda-cli
cargo build --release
# Checking if library is present
ls target/release/bloda
```

## WebAssembly

No web assembly support since we need file IO

## Contribution

Found a problem or have a suggestion? Feel free to open an issue.

## License

idk