use std::{collections::HashMap, fs, io::{self, Read, Seek, Write}, path::{Path, PathBuf}, sync::{Arc, Mutex}};

use diesel::{Connection, QueryDsl, RunQueryDsl, SelectableHelper};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use sql_structs::{ArchiveBlockInfo, ArchiveFileEntry, ArchiveFolderLeafEntry};

const DEFAULT_BLOCK_SIZE: u64 = 64 * 1024 * 1024; // 64MB
const DEFAULT_MAX_MEM_EXTRACT_SIZE: u64 = 16 * 1024 * 1024; // 16MB

mod compress_utils;
mod sql_structs;

pub struct ArchiveReader{
  archive_path: PathBuf,
  max_mem_extract_size: i64,
  files: HashMap<String, sql_structs::ArchiveFileEntry>,
  folder_leaves: HashMap<String, sql_structs::ArchiveFolderLeafEntry>,
  block_infos: Vec<sql_structs::ArchiveBlockInfo>,
}

impl ArchiveReader{
  pub fn new(archive_path: &Path, max_mem_extract_size: Option<u64>) -> Result<Self, String>{
    let max_mem_extract_size = max_mem_extract_size.unwrap_or(DEFAULT_MAX_MEM_EXTRACT_SIZE) as i64;
    // Extract index DB
    let mut fr = fs::File::open(archive_path)
      .map_err(|e| format!("at opening {archive_path:?}: {e}"))?;
    let mut index_len_bytes = [0u8; 8];
    fr.read(&mut index_len_bytes).map_err(|e| format!("at reading header size: {e}"))?;
    let index_len = u64::from_be_bytes(index_len_bytes);
    let mut index_compresses_data = vec![0u8; index_len as usize];
    let temp_file = tempfile::NamedTempFile::with_suffix(".db")
      .map_err(|e| format!("at creating temp index db file: {e}"))?;
    fr.read(&mut index_compresses_data).map_err(|e| format!("at reading header: {e}"))?;
    let mut index_data = vec![];
    compress_utils::decompress_data(&index_compresses_data[..], &mut index_data, "LZ4")
      .map_err(|e| format!("at decompressing index data: {e}"))?;
    fs::write(temp_file.path(), &index_data)
      .map_err(|e| format!("at writing header temp file: {e}"))?;

    let blob_offset = index_len + 8;
    // Load header DB
    let mut conn =
      diesel::SqliteConnection::establish(&temp_file.path().to_string_lossy().to_string())
      .map_err(|e| format!("at opening {:?}: {e}", temp_file.path()))?;
    
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
    let mut blocks = sql_structs::blocks::table
      .select(sql_structs::ArchiveBlockInfo::as_select())
      .load(&mut conn)
      .map_err(|e| format!("at getting block infos: {e}"))?;
    for block in blocks.iter_mut(){
      block.offset += blob_offset as i64;
    }

    Ok(Self {
      archive_path: archive_path.to_owned(),
      max_mem_extract_size,
      files: file_infos,
      folder_leaves: folder_leaf_infos,
      block_infos: blocks,
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

  fn extract_block_mem(&self, block_id: i64) -> Result<Vec<u8>, String>{
    let block_info = &self.block_infos[block_id as usize];
    let block_offset =  block_info.offset as u64;
    let block_size = block_info.size;
    let compression = &block_info.compression_type;
    let mut comp_data = vec![0u8; block_size as usize];
    let mut fr = fs::File::open(&self.archive_path)
      .map_err(|e| format!("at opening archive {:?}: {e}", &self.archive_path))?;
    fr
      .seek(io::SeekFrom::Start(block_offset))
      .map_err(|e| format!("at seeking to {block_offset}: {e}"))?;
    fr
      .read(&mut comp_data)
      .map_err(|e| format!("at reading blob {:?}: {e}", &self.archive_path))?;
    let mut raw_block_data = Vec::with_capacity(comp_data.len());
    compress_utils::decompress_data(&comp_data[..], &mut raw_block_data, compression)?;
    Ok(raw_block_data)
  }

  fn extract_block_file(&self, block_id: i64, out_file: &Path) -> Result<(), String>{
    let block_info = &self.block_infos[block_id as usize];
    let block_offset =  block_info.offset as u64;
    let block_size = block_info.size;
    let compression = &block_info.compression_type;
    let mut comp_data = vec![0u8; block_size as usize];
    let mut fr = fs::File::open(&self.archive_path)
      .map_err(|e| format!("at opening archive {:?}: {e}", &self.archive_path))?;
    fr
      .seek(io::SeekFrom::Start(block_offset))
      .map_err(|e| format!("at seeking to {block_offset}: {e}"))?;
    fr
      .read(&mut comp_data)
      .map_err(|e| format!("at reading blob {:?}: {e}", &self.archive_path))?;
    let mut fw = fs::File::create(out_file).map_err(|e| format!("at opening tempfile: {e}"))?;
    compress_utils::decompress_data(&comp_data[..], &mut fw, compression)?;
    Ok(())
  }

  pub fn extract_file(&self, name: &str, output: &Path) -> Result<(), String>{
    let file_info = self.files.get(name).ok_or(format!("{name} doesn't exist in archive"))?;
    if let Some(parent_dir) = output.parent(){
      fs::create_dir_all(parent_dir)
        .map_err(|e| format!("at creating dir {parent_dir:?}: {e}"))?;
    }
    let mut fw = fs::File::create(output).map_err(|e| format!("at opening {output:?}: {e}"))?;
    let block_size = &self.block_infos[file_info.block as usize];
    if block_size.size > self.max_mem_extract_size {
      let t_file = tempfile::NamedTempFile::new()
        .map_err(|e| format!("at creating tempfile: {e}"))?;
      self.extract_block_file(file_info.block, t_file.path())?;
      let mut fr = fs::File::open(t_file.path())
        .map_err(|e| format!("at opening temp file: {e}"))?;
      fr
        .seek(io::SeekFrom::Start(file_info.offset as u64))
        .map_err(|e| format!("at seeking in tempfile: {e}"))?;
      let mut fr = fr.take(file_info.size as u64);
      io::copy(&mut fr, &mut fw).map_err(|e| format!("at writing :{e}"))?;
    } else {
      let block_data = self.extract_block_mem(file_info.block)?;
      let start = file_info.offset as usize;
      let end = start + file_info.size as usize;
      fw.write(&block_data[start..end]).map_err(|e| format!("at writing :{e}"))?;
    }
    fw.flush().map_err(|e| format!("at flushing: {e}"))?;
    Ok(())
  }

  pub fn extract_files(
    &self,
    re_pattern: &str,
    output_dir: &Path,
    ignore_errors: bool
  ) -> Result<(), String>{
    let re_obj = regex::Regex::new(re_pattern).map_err(|e| format!("invalid regex: {e}"))?;

    self
      .folder_leaves
      .iter()
      .filter(|x| re_obj.is_match(x.0))
      .map(|x| output_dir.join(&x.0))
      .map(|x| fs::create_dir_all(&x).map_err(|e| format!("at creating leaf dir {:?}: {e}", &x)))
      .collect::<Result<(), String>>()?;

    let files_to_extract = self
      .files
      .iter()
      .filter(|x| re_obj.is_match(x.0))
      .map(|x| x.1)
      .collect::<Vec<_>>();

    let mut files_per_block = HashMap::new();
    for file_info in files_to_extract{
      files_per_block.entry(file_info.block).or_insert(vec![]).push(file_info);
    }

    for (block_id, file_infos) in files_per_block{
      let block_size = &self.block_infos[block_id as usize];
      if block_size.size > self.max_mem_extract_size {
        let t_file = tempfile::NamedTempFile::new()
          .map_err(|e| format!("at creating tempfile: {e}"))?;
        self.extract_block_file(block_id, t_file.path())?;
        for file_info in file_infos{
          let mut fr = fs::File::open(t_file.path())
            .map_err(|e| format!("at opening temp file: {e}"))?;
          let file_out_path = output_dir.join(&file_info.name);
          if let Some(file_out_dir) = file_out_path.parent(){
            fs::create_dir_all(file_out_dir)
              .map_err(|e| format!("at creating parent dir {file_out_dir:?}: {e}"))?;
          }
          let mut fw = fs::File::create(&file_out_path)
            .map_err(|e| format!("at opening {:?}: {e}", &file_out_path))?;
          fr
            .seek(io::SeekFrom::Start(file_info.offset as u64))
            .map_err(|e| format!("at seeking in tempfile: {e}"))?;
          let mut fr = fr.take(file_info.size as u64);
          io::copy(&mut fr, &mut fw).map_err(|e| format!("at writing: {e}"))?;
          fw.flush().map_err(|e| format!("at flushing: {e}"))?;
        }
      } else {
        let block_data = self.extract_block_mem(block_id)?;
        for file_info in file_infos{
          let file_out_path = output_dir.join(&file_info.name);
          if let Some(file_out_dir) = file_out_path.parent(){
            fs::create_dir_all(file_out_dir)
              .map_err(|e| format!("at creating parent dir {file_out_dir:?}: {e}"))?;
          }
          let mut fw = fs::File::create(&file_out_path)
            .map_err(|e| format!("at opening {:?}: {e}", &file_out_path))?;
          let start = file_info.offset as usize;
          let end = start + file_info.size as usize;
          fw.write(&block_data[start..end]).map_err(|e| format!("at writing :{e}"))?;
          fw.flush().map_err(|e| format!("at flushing: {e}"))?;
        }
      }
    }
    Ok(())
  }
}

fn write_index_data(
  db_path: &str,
  files: Vec<ArchiveFileEntry>,
  folder_leaves: Vec<ArchiveFolderLeafEntry>,
  block_infos: Vec<ArchiveBlockInfo>,
) -> Result<(), String>{
  if Path::new(db_path).is_file(){
    fs::remove_file(&db_path).map_err(|e| format!("at deleting existing db: {e}"))?;
  }
  let mut conn = diesel::SqliteConnection::establish(&db_path)
    .map_err(|e| format!("at opening {db_path}: {e}"))?;
  diesel::sql_query("CREATE TABLE files(
    name TEXT PRIMARY KEY,
    block BIGINT,
    offset BIGINT,
    size BIGINT)"
  )
    .execute(&mut conn)
    .map_err(|e| format!("at creating files table: {e}"))?;
  diesel::sql_query("CREATE TABLE folder_leaves(name TEXT PRIMARY KEY)")
    .execute(&mut conn)
    .map_err(|e| format!("at creating folder_leaves table: {e}"))?;
  diesel::sql_query("CREATE TABLE blocks(
    id BIGINT PRIMARY KEY,
    size BIGINT,
    offset BIGINT,
    compression_type TEXT)"
  )
    .execute(&mut conn)
    .map_err(|e| format!("at creating blocks table: {e}"))?;
  diesel::insert_into(sql_structs::files::table)
    .values(&files)
    .execute(&mut conn)
    .map_err(|e| format!("at writing files info: {e}"))?;
  diesel::insert_into(sql_structs::folder_leaves::table)
    .values(&folder_leaves)
    .execute(&mut conn)
    .map_err(|e| format!("at writing folder leaves info: {e}"))?;
  diesel::insert_into(sql_structs::blocks::table)
    .values(&block_infos)
    .execute(&mut conn)
    .map_err(|e| format!("at writing archive info: {e}"))?;
  Ok(())
}

fn distribute_files_to_blocks(
  inp_dir: &Path,
  max_multi_block_size: i64
) -> (Vec<Vec<(PathBuf, i64, i64)>>, Vec<PathBuf>) {
  let entries = walkdir::WalkDir::new(inp_dir)
    .into_iter()
    .filter_map(|x| x.ok())
    .map(|x| x.path().to_owned())
    .collect::<Vec<_>>();
  let mut files_w_sizes = entries
    .iter()
    .filter(|x| x.is_file())
    .filter_map(|x| x.metadata().map(|m| (x, m.len() as i64)).ok())
    .collect::<Vec<_>>();
  files_w_sizes.sort_by_key(|x| x.1);
  let folder_leaves = entries
    .iter()
    .filter(|x| x.is_dir() && fs::read_dir(*x).map(|mut y| y.next().is_none()).unwrap_or(false))
    .cloned()
    .collect::<Vec<_>>();

  let mut block_infos = vec![];

  let mut curr_block_files = vec![];
  let mut curr_block_offset = 0;
  for (path, size) in files_w_sizes{
    if (curr_block_offset + size > max_multi_block_size) && curr_block_files.len() > 0{
      block_infos.push(curr_block_files);
      curr_block_files = vec![];
      curr_block_offset = 0;
    }
    curr_block_files.push((path.clone(), curr_block_offset, size));
    curr_block_offset += size;
  }
  if curr_block_files.len() > 0{
    block_infos.push(curr_block_files);
  }
  (block_infos, folder_leaves)
}

fn compress_block(
  output: &Path,
  block_files: &[(PathBuf, i64, i64)],
  compression_type: &str
) -> Result<u64, String>{
  if block_files.len() == 1{
    if let Some((path, _, _)) = block_files.last(){
      let fr = fs::File::open(&path).map_err(|e| format!("at opening {:?}: {e}", &path))?;
      let mut fw = fs::File::create(output).map_err(|e| format!("at creating {output:?}: {e}"))?;
      return compress_utils::compress_data(fr, &mut fw, compression_type);
    } else {
      return Err("should not occur".to_string())
    }
  }
  let total_size = block_files.iter().map(|x| x.2).sum::<i64>();
  let mut block_data = vec![0u8; total_size as usize];
  for (path, offset, size) in block_files{
    let mut fr = fs::File::open(path).map_err(|e| format!("at opening {path:?}: {e}"))?;
    fr.read(&mut block_data[*offset as usize..(*offset + size) as usize])
      .map_err(|e| format!("at adding {path:?} to buffer: {e}"))?;
  }
  let mut compressed_block_data = Vec::<u8>::new();
  compress_utils::compress_data(&block_data[..], &mut compressed_block_data, compression_type)?;
  fs::write(output, &compressed_block_data).map_err(|e| format!("at writing: {e}"))?;
  Ok(compressed_block_data.len() as _)
} 

fn create_archive_inner(
  dir: &Path,
  output: &Path,
  compression_type: &str,
  max_multi_block_size: Option<u64>
) -> Result<(), String>{
  let max_multi_block_size = max_multi_block_size.unwrap_or(DEFAULT_BLOCK_SIZE) as i64;
  let (block_files, folder_leaves) = distribute_files_to_blocks(dir, max_multi_block_size);

  let block_sizes = block_files
    .iter()
    .enumerate()
    .map(|(i, x)| {
      let block_path = output.with_extension(format!("temp.{i}"));
      compress_block(&block_path, x, compression_type)
    })
    .collect::<Result<Vec<u64>, String>>()?;

  let mut block_infos = vec![];
  let mut curr_offset = 0;
  for (i, size) in block_sizes.iter().enumerate(){
    block_infos.push(ArchiveBlockInfo{
      id: i as _,
      size: *size as _,
      offset: curr_offset,
      compression_type: compression_type.to_string()
    });
    curr_offset += *size as i64;
  }
  let folder_leaf_infos = folder_leaves
    .iter()
    .map(|x| ArchiveFolderLeafEntry{
      name: x.strip_prefix(dir).unwrap_or(x).to_string_lossy().to_string()
    })
    .collect::<Vec<_>>();
  let mut file_infos = vec![];
  for (i, in_files) in block_files.iter().enumerate(){
    for (path, offset, size) in in_files{
      let entry_name = path.strip_prefix(dir).unwrap_or(path).to_string_lossy().to_string();
      file_infos.push(ArchiveFileEntry{
        name: entry_name,
        block: i as _,
        offset: *offset,
        size: *size
      });
    }
  }
  let blob_path = output.with_extension("bdablob");
  let mut fw = fs::File::create(&blob_path).map_err(|e| format!("at creating blob: {e}"))?;
  for i in 0..block_infos.len(){
    let block_path = output.with_extension(format!("temp.{i}"));
    let mut fr = fs::File::open(&block_path).map_err(|e| format!("at reading block: {e}"))?;
    io::copy(&mut fr, &mut fw).map_err(|e| format!("at writing blob: {e}"))?;
    let _ = fs::remove_file(&block_path)
      .inspect_err(|e| eprintln!("at removing temp file {:?}: {e}", &block_path));
  }
  fw.flush().map_err(|e| format!("at flushing blob: {e}"))?;

  let db_path_name = output.with_extension("bdadb").to_string_lossy().to_string();
  write_index_data(&db_path_name, file_infos, folder_leaf_infos, block_infos)
    .map_err(|e| format!("at making index db: {e}"))?;

  let mut fw = fs::File::create(output)
    .map_err(|e| format!("at opening output file {output:?}: {e}"))?;
  let mut compressed_index = Vec::<u8>::new();
  let fr = fs::File::open(&db_path_name).map_err(|e| format!("at reading index db: {e}"))?;
  compress_utils::compress_data(fr, &mut compressed_index, compression_type)?;
  fw
    .write(&compressed_index.len().to_be_bytes())
    .map_err(|e| format!("at writing index len: {e}"))?;
  fw.write(&compressed_index).map_err(|e| format!("at writing index: {e}"))?;
  let mut fr = fs::File::open(&blob_path).map_err(|e| format!("at reading blob: {e}"))?;
  io::copy(&mut fr, &mut fw).map_err(|e| format!("at writing blob: {e}"))?;
  fw.flush().map_err(|e| format!("at flushing to output: {e}"))?;
  let _ = fs::remove_file(&db_path_name).inspect_err(|e| eprintln!("at removing index file: {e}"));
  let _ = fs::remove_file(&blob_path).inspect_err(|e| eprintln!("at removing blob file: {e}"));
  Ok(())
}

pub fn create_archive(
  dir: &Path,
  output: &Path,
  compression_type: &str,
  threads: u8,
  block_size: Option<u64>
) -> Result<(), String>{
  let t_pool = rayon::ThreadPoolBuilder::new()
    .num_threads(threads as _)
    .build()
    .map_err(|e| format!("at creating thread pool: {e}"))?;
  t_pool.install(|| {create_archive_inner(dir, output, compression_type, block_size)})
}

pub fn decompress_archive(bda_path: &Path, out_dir: &Path) -> Result<(), String>{
  let archive = ArchiveReader::new(bda_path, None).map_err(|e| format!("invalid archive: {e}"))?;
  archive.extract_files(".*", out_dir, true).map_err(|e| format!("at extracting: {e}"))?;
  Ok(())
}
