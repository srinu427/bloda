use diesel::prelude::{Insertable, Queryable, Selectable};

diesel::table! {
  files (name) {
    name -> Text,
    block -> BigInt,
    offset -> BigInt,
    size -> BigInt,
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
    size -> BigInt,
    offset -> BigInt,
    compression_type -> Text,
  }
}

#[derive(Debug, Clone)]
#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = files)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ArchiveFileEntry{
  pub name: String,
  pub block: i64,
  pub offset: i64,
  pub size: i64,
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
  pub size: i64,
  pub offset: i64,
  pub compression_type: String,
}