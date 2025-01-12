use std::{fs, io::{self, Read, Seek, Write}, path::{Path, PathBuf}};

use diesel::{Connection, RunQueryDsl};
use sql_structs::{ArchiveBlockInfo, ArchiveFileEntry, ArchiveFolderLeafEntry};
use walkdir::WalkDir;

const DEFAULT_BLOCK_SIZE: u32 = 4 * 1024 * 1024; // 4MB

mod compress_utils;
mod sql_structs;


fn create_header_and_work(
  dir: &Path,
  block_size: i32,
) -> (Vec<ArchiveFileEntry>, Vec<ArchiveFolderLeafEntry>, Vec<Vec<(String, PathBuf, i32)>>){
  let dir_entry_list = WalkDir::new(dir)
    .into_iter()
    .filter_map(|x| x.inspect_err(|e| eprintln!("error listing entry: {e}. skipping it")).ok())
    .map(|x| x.into_path())
    .collect::<Vec<_>>();
  let files = dir_entry_list.iter().filter(|x| x.is_file()).cloned().collect::<Vec<_>>();
  let leaf_dirs = dir_entry_list
    .iter()
    .filter(|x| fs::read_dir(x).map(|mut y| y.next().is_some()).unwrap_or(false))
    .map(|x| x.strip_prefix(dir).unwrap_or(x))
    .map(|x| x.to_string_lossy().to_string())
    .map(|x| ArchiveFolderLeafEntry{name: x})
    .collect::<Vec<_>>();
  
  let mut file_entry_info_map = files
    .iter()
    .filter_map(|x| Some((
      x,
      fs::metadata(x)
        .map(|m| m.len() as i64)
        .inspect_err(|e| eprintln!("error getting size of {:?}: {e}. skipping it", x))
        .ok()?
    )))
    .collect::<Vec<(_, _)>>();
  file_entry_info_map.sort_by_key(|a| a.1);

  let total_size: i64 = file_entry_info_map.iter().map(|x| x.1).sum();
  let block_count = ((total_size - 1) / block_size as i64) + 1;
  
  let mut archive_file_entries = Vec::with_capacity(file_entry_info_map.len());
  let mut block_file_infos = vec![vec![]; block_count as _];
  let mut curr_block_no = 0;
  let mut curr_block_offset = 0;

  for (path, size) in file_entry_info_map {
    let entry_name = path.strip_prefix(dir).unwrap_or(path).to_string_lossy().to_string();
    archive_file_entries.push(ArchiveFileEntry{
      name: entry_name.clone(),
      size,
      start_block: curr_block_no,
      start_offset: curr_block_offset
    });
    let mut rem_file_size = size;
    loop {
      block_file_infos[curr_block_no as usize]
        .push((entry_name.clone(), path.clone(), curr_block_offset));
      let rem_block_size = block_size - curr_block_offset;
      if rem_block_size as i64 > rem_file_size{
        curr_block_offset += rem_file_size as i32;
        break;
      } else {
        curr_block_no += 1;
        curr_block_offset = 0;
        rem_file_size -= rem_block_size as i64;
      }
    }
  }
  (archive_file_entries, leaf_dirs, block_file_infos)
}

pub fn create_archive(
  dir: &Path,
  output: &Path,
  compression_type: &str,
  threads: u8,
  block_size: Option<u32>
) -> Result<(), String>{
  let block_size = block_size.unwrap_or(DEFAULT_BLOCK_SIZE) as i32;
  let (files, folder_leaves, work) = create_header_and_work(dir, block_size);

  let block_temp_file_prefix = format!("{}.tempblock", output.to_string_lossy());
  for (block_id, block_info) in work.iter().enumerate() {
    let mut block = vec![0u8; block_size as usize];
    let mut block_filled_len = 0;
    for (_, f_path, offset) in block_info{
      let mut fr =
        fs::File::open(&f_path).map_err(|e| format!("at opening: {:?}: {e}", &f_path))?;
      fr
        .seek(io::SeekFrom::Start(*offset as _))
        .map_err(|e| format!("at seeking to {offset} in {:?}: {e}", &f_path))?;
      let size_read = fr
        .read(&mut block[block_filled_len..])
        .map_err(|e| format!("at reading from {:?}: {e}", &f_path))?;
      block_filled_len += size_read;
    }
    block = block[..block_filled_len].to_vec();
    let block_file_name = PathBuf::from(format!("{block_temp_file_prefix}.{block_id}"));
    let compressed_data = if compression_type == "NONE"{
      block
    } else {
      compress_utils::compress_data(&block, compression_type)?
    };
    fs::write(&block_file_name, &compressed_data)
      .map_err(|e| format!("at writing to tempfile: {:?}: {e}", &block_file_name))?;
  }

  let blob_path = PathBuf::from(format!("{}.bdablob", output.to_string_lossy()));
  let mut fw = fs::File::create(&blob_path)
    .map_err(|e| format!("at opening {:?}: {e}", &blob_path))?;

  let block_sizes = Vec::with_capacity(work.len());
  for block_id in 0..work.len() {
    let block_file_name = PathBuf::from(format!("{block_temp_file_prefix}.{block_id}"));
    let mut fr = fs::File::open(&block_file_name)
      .map_err(|e| format!("at opening tempfile {:?}: {e}", &block_file_name))?;
    io::copy(&mut fr, &mut fw).map_err(|e| format!("at writing to blob: {e}"))?;
    fs::remove_file(&block_file_name)
      .map_err(|e| format!("at removing tempfile {:?}: {e}", &block_file_name))?;
  }

  let db_path = format!("{}.bdadb", output.to_string_lossy());
  let mut conn = diesel::SqliteConnection::establish(&db_path)
    .map_err(|e| format!("at opening {}: {e}", &db_path))?;
  diesel::sql_query("CREATE TABLE files(
    name TEXT PRIMARY KEY,
    size BIGINT,
    start_block BIGINT,
    start_offset INTEGER)"
  )
    .execute(&mut conn)
    .map_err(|e| format!("at creating files table in index: {e}"))?;
  diesel::insert_into(sql_structs::files::table)
    .values(&files)
    .execute(&mut conn)
    .map_err(|e| format!("at writing files info to index: {e}"))?;
  diesel::insert_into(sql_structs::folder_leaves::table)
    .values(&folder_leaves)
    .execute(&mut conn)
    .map_err(|e| format!("at writing folder leaves info to index: {e}"))?;
  diesel::insert_into(sql_structs::blocks::table)
    .values(
      &block_sizes
        .iter()
        .enumerate()
        .map(|(i, x)| ArchiveBlockInfo{
          id: i as i64,
          size: *x,
          compression_type: compression_type.to_string(),
          compression_level: 0
        })
        .collect::<Vec<_>>()
    )
    .execute(&mut conn)
    .map_err(|e| format!("at writing archive info to index: {e}"))?;

  Ok(())
}
