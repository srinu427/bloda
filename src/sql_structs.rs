use diesel::prelude::{Insertable, Queryable, Selectable};

diesel::table! {
  files (name) {
    name -> Text,
    size -> BigInt,
    start_block -> BigInt,
    start_offset -> Integer,
  }
}

diesel::table! {
  folder_leaves (name) {
    name -> Text,
  }
}

diesel::table! {
  blocks (id) {
    id -> BigInt,
    size -> Integer,
    original_size -> Integer,
    compression_type -> Text,
    compression_level -> Integer
  }
}

#[derive(Debug, Clone)]
#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = files)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ArchiveFileEntry{
  pub name: String,
  pub size: i64,
  pub start_block: i64,
  pub start_offset: i32,
}

#[derive(Debug, Clone)]
#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = folder_leaves)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ArchiveFolderLeafEntry{
  pub name: String
}

#[derive(Debug, Clone)]
#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = blocks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ArchiveBlockInfo{
  pub id: i64,
  pub size: i32,
  pub original_size: i32,
  pub compression_type: String,
  pub compression_level: i32,
}