use std::io::{self, Read, Write};

pub fn decompress_data<R: Read, W: Write>(
  input_stream: R,
  output_stream: &mut W,
  compression: &str
) -> Result<u64, String>{
  match compression {
    "LZMA" => {
      let mut reader = lzma::LzmaReader::new_decompressor(input_stream)
        .map_err(|e| format!("at starting lzma reader: {e}"))?;
      let size = io::copy(&mut reader, output_stream)
        .map_err(|e| format!("at decompressing: {e}"))?;
      output_stream.flush().map_err(|e| format!("at flushing: {e}"))?;
      Ok(size)
    }
    "LZ4" => {
      let mut reader = lz4_flex::frame::FrameDecoder::new(input_stream);
      let size = io::copy(&mut reader, output_stream)
        .map_err(|e| format!("at decompressing: {e}"))?;
      output_stream.flush().map_err(|e| format!("at flushing: {e}"))?;
      Ok(size)
    },
    "ZSTD" => {
      let mut reader = zstd::Decoder::new(input_stream)
        .map_err(|e| format!("at initializing zstd decompressor: {e}"))?;
      let size = io::copy(&mut reader, output_stream)
        .map_err(|e| format!("at decompressing: {e}"))?;
      output_stream.flush().map_err(|e| format!("at flushing: {e}"))?;
      Ok(size)
    },
    _ => {
      return Err("unknown compression type".to_string());
    }
  }
}

pub fn compress_data<R: Read, W: Write>(
  mut input_data: R,
  output_stream: &mut W,
  compression: &str
) -> Result<u64, String> {
  match compression {
    "LZMA" => {
      let mut writer = lzma::LzmaWriter::new_compressor(output_stream, 9)
        .map_err(|e| format!("at starting lzma writer: {e}"))?;
      let size = io::copy(&mut input_data, &mut writer)
        .map_err(|e| format!("at compressing: {e}"))?;
      writer.finish().map_err(|e| format!("at finishing: {e}"))?;
      Ok(size)
    }
    "LZ4" => {
      let mut writer = lz4_flex::frame::FrameEncoder::new(output_stream);
      let size = io::copy(&mut input_data, &mut writer)
        .map_err(|e| format!("at compressing: {e}"))?;
      writer.finish().map_err(|e| format!("at flushing: {e}"))?;
      Ok(size)
    },
    "ZSTD" => {
      let mut writer = zstd::stream::Encoder::new(output_stream, 6)
        .map_err(|e| format!("at initializing zstd compressor: {e}"))?;
      let size = io::copy(&mut input_data, &mut writer)
        .map_err(|e| format!("at compressing: {e}"))?;
      writer.finish().map_err(|e| format!("at finishing: {e}"))?;
      Ok(size)
    },
    _ => {
      return Err("unknown compression type".to_string());
    }
  }
}