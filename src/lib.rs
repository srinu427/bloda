use std::{collections::HashMap, fs, io::{self, Read, Seek, Write}, path::{Path, PathBuf}};

use diesel::{Connection, QueryDsl, RunQueryDsl, SelectableHelper};
use sql_structs::{ArchiveBlockInfo, ArchiveFileEntry, ArchiveFolderLeafEntry};
use walkdir::WalkDir;

const DEFAULT_BLOCK_SIZE: u32 = 4 * 1024 * 1024; // 4MB

mod compress_utils;
mod sql_structs;

pub struct ArchiveReader{
  header_path: PathBuf,
  blob_path: PathBuf,
  files: HashMap<String, sql_structs::ArchiveFileEntry>,
  folder_leaves: HashMap<String, sql_structs::ArchiveFolderLeafEntry>,
  block_infos: Vec<sql_structs::ArchiveBlockInfo>,
  block_offsets: Vec<u64>,
}

impl ArchiveReader{
  pub fn new(header: &Path, blob: &Path) -> Result<Self, String>{
    let mut conn = diesel::SqliteConnection::establish(&header.to_string_lossy().to_string())
      .map_err(|e| format!("at opening {:?}: {e}", header))?;
    
    let file_infos = sql_structs::files::table
      .select(sql_structs::ArchiveFileEntry::as_select())
      .load(&mut conn)
      .map_err(|e| format!("at getting file infos: {e}"))?
      .iter()
      .cloned()
      .map(|x| (x.name.clone(), x))
      .collect();
    let folder_leaf_infos = sql_structs::folder_leaves::table
      .select(sql_structs::ArchiveFolderLeafEntry::as_select())
      .load(&mut conn)
      .map_err(|e| format!("at getting folder leaf infos: {e}"))?
      .iter()
      .cloned()
      .map(|x| (x.name.clone(), x))
      .collect();
    let blocks = sql_structs::blocks::table
      .select(sql_structs::ArchiveBlockInfo::as_select())
      .load(&mut conn)
      .map_err(|e| format!("at getting block infos: {e}"))?;

    println!("blocks: {blocks:?}");

    let block_offsets = (0..blocks.len())
      .map(|i| blocks[0..i].iter().map(|x| x.size as u64).sum::<u64>())
      .collect::<Vec<_>>();

    println!("block_offsets: {block_offsets:?}");

    Ok(Self {
      header_path: header.to_owned(),
      blob_path: blob.to_owned(),
      files: file_infos,
      folder_leaves: folder_leaf_infos,
      block_infos: blocks,
      block_offsets,
    })
  }

  pub fn list_all_entries(&self) -> Vec<String>{
    let mut  dir_leaves = self
      .folder_leaves
      .values()
      .map(|x| x.name.clone())
      .collect::<Vec<_>>();
    let mut files = self
      .files
      .values()
      .map(|x| x.name.clone())
      .collect::<Vec<_>>();
    files.append(&mut dir_leaves);
    files
  }

  pub fn list_entries(&self, regex_pattern: &str) -> Result<Vec<String>, String>{
    let re = regex::Regex::new(regex_pattern).map_err(|e| format!("invalid re pattern: {e}"))?;
    let mut  dir_leaves = self
      .folder_leaves
      .values()
      .filter(|x| re.is_match(&x.name))
      .map(|x| x.name.clone())
      .collect::<Vec<_>>();
    let mut files = self
      .files
      .values()
      .filter(|x| re.is_match(&x.name))
      .map(|x| x.name.clone())
      .collect::<Vec<_>>();
    files.append(&mut dir_leaves);
    Ok(files)
  }

  fn extract_block(&self, block_id: i64) -> Result<Vec<u8>, String>{
    let block_offset =  self.block_offsets[block_id as usize];
    let block_size = self.block_infos[block_id as usize].size;
    let compression = &self.block_infos[block_id as usize].compression_type;
    let mut comp_data = vec![0u8; block_size as usize];
    let mut fr = fs::File::open(&self.blob_path)
      .map_err(|e| format!("at opening blob {:?}: {e}", &self.blob_path))?;
    fr
      .seek(io::SeekFrom::Start(block_offset))
      .map_err(|e| format!("at seeking to {block_offset}: {e}"))?;
    fr
      .read(&mut comp_data)
      .map_err(|e| format!("at reading blob {:?}: {e}", &self.blob_path))?;
    compress_utils::decompress_data(&mut comp_data, compression)
  }

  pub fn extract_file(&self, name: &str, output: &Path) -> Result<(), String>{
    let file_info = self.files.get(name).ok_or(format!("{name} doesn't exist in archive"))?;
    if let Some(parent_dir) = output.parent(){
      fs::create_dir_all(parent_dir)
        .map_err(|e| format!("at creating dir {parent_dir:?}: {e}"))?;
    }
    let mut fw = fs::File::create(output).map_err(|e| format!("at opening {output:?}: {e}"))?;
    for block_id in file_info.start_block..file_info.end_block + 1{
      let block_data = self.extract_block(block_id)
        .map_err(|e| format!("at extracting block: {block_id}: {e}"))?;
      let slice_to_write = if block_id == file_info.start_block{
        &block_data[file_info.start_offset as usize..]
      } else if block_id == file_info.end_block {
        &block_data[..file_info.end_offset as usize]
      } else {
        &block_data
      };
      fw.write(slice_to_write)
        .map_err(|e| format!("at writing from block: {block_id}: {e}"))?;
    }
    fw.flush().map_err(|e| format!("at flushing to {output:?}: {e}"))?;
    Ok(())
  }

  pub fn extract_files(
    &self,
    re_pattern: &str,
    output_dir: &Path,
    ignore_errors: bool
  ) -> Result<(), String>{
    let re_obj = regex::Regex::new(re_pattern).map_err(|e| format!("invalid regex: {e}"))?;
    let mut files_to_extract =
      self.files.iter().filter(|x| re_obj.is_match(x.0)).map(|x| x.1).collect::<Vec<_>>();
    files_to_extract.sort_by_key(|x| x.start_block);

    let mut per_start_block = HashMap::new();
    for file_info in files_to_extract{
      let val = per_start_block
        .entry(file_info.start_block)
        .or_insert((vec![], None));
      if file_info.start_block != file_info.end_block{
        val.1 = Some(file_info)
      } else {
        val.0.push(file_info);
      }
    }

    for (block_id, (work, multi_block_work)) in per_start_block{
      let start_block_data = self
        .extract_block(block_id)
        .map_err(|e| format!("at reading block {block_id}: {e}"))?;
      for file_info in work{
        let out_name = output_dir.join(&file_info.name);
        if let Some(parent_dir) = out_name.parent(){
          fs::create_dir_all(parent_dir)
            .map_err(|e| format!("at creating dir {parent_dir:?}: {e}"))?;
        }
        let mut fw = fs::File::create(&out_name)
          .map_err(|e| format!("at opening {:?}: {e}", &out_name))?;
        println!("file_info: {file_info:?}");
        fw
          .write(&start_block_data[file_info.start_offset as usize..file_info.end_offset as _])
          .map_err(|e| format!("at writing to {:?}: {e}", &out_name))?;
        fw.flush().map_err(|e| format!("at flushing to {:?}: {e}", &out_name))?;
      }
      if let Some(file_info) = multi_block_work{
        let out_name = output_dir.join(&file_info.name);
        if let Some(parent_dir) = out_name.parent(){
          fs::create_dir_all(parent_dir)
            .map_err(|e| format!("at creating dir {parent_dir:?}: {e}"))?;
        }
        let mut fw = fs::File::create(&out_name)
          .map_err(|e| format!("at opening {:?}: {e}", &out_name))?;
        fw
          .write(&start_block_data[file_info.start_offset as usize..])
          .map_err(|e| format!("at writing to {:?}: {e}", &out_name))?;
        for block_id in file_info.start_block + 1..file_info.end_block + 1{
          let block_data = self.extract_block(block_id)
            .map_err(|e| format!("at extracting block: {block_id}: {e}"))?;
          let slice_to_write = if block_id == file_info.start_block{
            &block_data[file_info.start_offset as usize..]
          } else if block_id == file_info.end_block {
            &block_data[..file_info.end_offset as usize]
          } else {
            &block_data
          };
          fw.write(slice_to_write)
            .map_err(|e| format!("at writing from block: {block_id}: {e}"))?;
        }
        fw.flush().map_err(|e| format!("at flushing to {:?}: {e}", &out_name))?;
      }
    }
    Ok(())
  }
}

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
    let start_block = curr_block_no;
    let start_offset = curr_block_offset;
    let entry_name = path.strip_prefix(dir).unwrap_or(path).to_string_lossy().to_string();
    
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

    archive_file_entries.push(ArchiveFileEntry{
      name: entry_name.clone(),
      start_block,
      start_offset,
      end_block: curr_block_no,
      end_offset: curr_block_offset,
    });
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

  let mut block_sizes = Vec::with_capacity(work.len());
  for block_id in 0..work.len() {
    let block_file_name = PathBuf::from(format!("{block_temp_file_prefix}.{block_id}"));
    let mut fr = fs::File::open(&block_file_name)
      .map_err(|e| format!("at opening tempfile {:?}: {e}", &block_file_name))?;
    let block_size = io::copy(&mut fr, &mut fw).map_err(|e| format!("at writing to blob: {e}"))?;
    block_sizes.push(block_size as i32);
    fs::remove_file(&block_file_name)
      .map_err(|e| format!("at removing tempfile {:?}: {e}", &block_file_name))?;
  }

  let db_path = format!("{}.bdadb", output.to_string_lossy());
  let mut conn = diesel::SqliteConnection::establish(&db_path)
    .map_err(|e| format!("at opening {}: {e}", &db_path))?;
  diesel::sql_query("CREATE TABLE files(
    name TEXT PRIMARY KEY,
    start_block BIGINT,
    start_offset INTEGER,
    end_block BIGINT,
    end_offset INTEGER)"
  )
    .execute(&mut conn)
    .map_err(|e| format!("at creating files table in index: {e}"))?;
  diesel::sql_query("CREATE TABLE folder_leaves(name TEXT PRIMARY KEY)")
    .execute(&mut conn)
    .map_err(|e| format!("at creating folder_leaves table in index: {e}"))?;
  diesel::sql_query("CREATE TABLE blocks(
    id BIGINT PRIMARY KEY,
    size INTEGER,
    compression_type TEXT,
    compression_level INTEGER)"
  )
    .execute(&mut conn)
    .map_err(|e| format!("at creating blocks table in index: {e}"))?;
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

pub fn decompress_archive(bdadb: &Path, bdablob: &Path, out_dir: &Path) -> Result<(), String>{
  let archive = ArchiveReader::new(bdadb, bdablob).map_err(|e| format!("invalid archive: {e}"))?;
  archive.extract_files(".*", out_dir, true).map_err(|e| format!("at extracting: {e}"))?;
  Ok(())
}
