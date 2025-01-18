# BLOcked Directory Archive Python Library


An archival tool to make handling directories with huge number of small files in Network file systems better.

## Usage

WIP

## Installing

You need a Python 3.8+ environment to build and install the library. You can follow the instructions in the [official python wiki](https://wiki.python.org/moin/BeginnersGuide/Download) to download Python.

Virtual environments are recommended to not pollute your global Python installation, you can follow the instructions from this official [tutorial](https://docs.python.org/3/tutorial/venv.html)

You also need a working `Rust` and `Cargo` setup. [Rustup](https://rustup.rs/) is the simplest way to set this up on either Windows, Mac or Linux.

You will need libsqlite3 installed during build/install time.
You will also optionally need liblzma and libzstd installed to use those compression formats in runtime.

Once the prerequisites have been installed, and your favourite Python virtual environment is 'sourced', run the following command to install the package:

```
git clone https://github.com/srinu427/bloda.git
cd bloda/bloda-pyo3
pip install .
```

## WebAssembly

No web assembly support since we need file IO

## Contribution

Found a problem or have a suggestion? Feel free to open an issue.

## License

idk
