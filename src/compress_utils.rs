use std::io::Write;

pub fn compress_data(input_data: &[u8], compression: &str) -> Result<Vec<u8>, String> {
  let output_data = match compression {
    "LZMA" => {
      lzma::compress(&input_data, 9).map_err(|e| format!("at compressing block: {e}"))?
    }
    "LZ4" => {
      let compressed_data = Vec::with_capacity(input_data.len());
      let mut lz4_writer = lz4_flex::frame::FrameEncoder::new(compressed_data);
      lz4_writer.write_all(&input_data).map_err(|e| format!("at compressing block: {e}"))?;
      lz4_writer.finish().map_err(|e| format!("at compressing block: {e}"))?
    },
    "ZSTD" => {
      let compressed_data = Vec::with_capacity(input_data.len());
      let mut zstd_writer = zstd::Encoder::new(compressed_data, 5)
        .map_err(|e| format!("at initializing zstd compressor: {e}"))?;
      zstd_writer.set_pledged_src_size(Some(input_data.len() as _))
        .map_err(|e| format!("at setting inp len: {e}"))?;
      zstd_writer.include_contentsize(true)
        .map_err(|e| format!("at setting inp len include: {e}"))?;
      zstd_writer.write_all(&input_data).map_err(|e| format!("at compressing block: {e}"))?;
      zstd_writer.finish().map_err(|e| format!("at compressing block: {e}"))?
    },
    _ => {
      return Err("unknown compression type".to_string());
    }
  };
  Ok(output_data)
}