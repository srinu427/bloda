use std::{error::Error, path::PathBuf};

use clap::{arg, Args, Parser, Subcommand};

#[derive(Args)]
struct CompressArgs {
  /// Input directory name. If a file is provided, empty archive is generated
  #[arg(long, short = 'i')]
  input_path: PathBuf,
  /// Output file's name. Expected extention name is .bda
  #[arg(long, short = 'o')]
  output_path: PathBuf,
  /// Number of block to compress in parallel
  #[arg(long, short = 't', default_value_t = 1)]
  thread_count: u8,
  /// Compression to use. Defaults to ZSTD
  /// supported: LZMA, LZ4, ZSTD, NONE
  #[arg(long, short = 'c', default_value_t = String::from("ZSTD"))]
  compression: String,
  /// Max size of file in bytes to be processed in memory instead of writing to temp file.
  /// Use 0 to reduce RAM usage
  #[arg(long, short = 'b', default_value_t = 64 * 1024 * 1024)]
  block_size: u64,
}

#[derive(Args)]
struct DecompressArgs {
  /// Input archive name. Expecting a .bda file
  #[arg(long, short = 'i')]
  input_arc: PathBuf,
  /// Output Dir name. Will be created if not present
  #[arg(long, short = 'o')]
  output_dir: PathBuf,
  /// Number of block to compress in parallel
  #[arg(long, short = 't', default_value_t = 1)]
  thread_count: u8,
}

#[derive(Subcommand)]
enum AppCommands {
  Compress(CompressArgs),
  Decompress(DecompressArgs),
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct AppArgs {
  #[command(subcommand)]
  command: AppCommands,
}

fn main() -> Result<(), Box<dyn Error>>{
  let args = AppArgs::parse();
  match args.command {
    AppCommands::Compress(compress_args) => {
      let _ = bloda_sys::create_archive(
        &compress_args.input_path,
        &compress_args.output_path,
        &compress_args.compression,
        compress_args.thread_count,
        Some(compress_args.block_size)
      )
        .inspect_err(|e| eprintln!("error: {e}"))?;
    },
    AppCommands::Decompress(decompress_args) => {
      let _ = bloda_sys::decompress_archive(
        &decompress_args.input_arc,
        &decompress_args.output_dir
      )
        .inspect_err(|e| eprintln!("error: {e}"))?;
    },
  }
  Ok(())
}
