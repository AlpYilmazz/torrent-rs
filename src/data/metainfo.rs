use std::{
    io::{BufReader, Read},
    path::Path,
};

use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

use crate::util;

#[derive(Debug, thiserror::Error)]
pub enum TorrentFileError {
    #[error("Deserialization Error: {0:?}")]
    ParseError(#[from] serde_bencode::Error),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metainfo {
    pub info: Info,
    pub announce: String,
    #[serde(rename = "announce-list")]
    pub announce_list: Option<Vec<Vec<String>>>,
    #[serde(rename = "creation date")]
    pub creation_date: Option<u64>,
    pub comment: Option<String>,
    #[serde(rename = "created by")]
    pub created_by: Option<String>,
    pub encoding: Option<String>,
}

impl Metainfo {
    pub fn from_torrent_file(path: impl AsRef<Path>) -> Result<Self, TorrentFileError> {
        let mut reader = BufReader::new(std::fs::File::open(path)?);
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(Self::from_bytes(&buf)?)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_bencode::Error> {
        // let bytes = if atty::is(atty::Stream::Stdin) {
        //     let second_arg = std::env::args_os()
        //         .nth(1)
        //         .ok_or("missing `path` argument".to_string())?;
        //     let mut file = std::fs::File::open(second_arg)?;
        //     let mut buf = Vec::new();
        //     file.read_to_end(&mut buf)?;
        //     buf
        // } else {
        //     let mut buf = Vec::new();
        //     std::io::stdin().lock().read_to_end(&mut buf)?;
        //     buf
        // };

        serde_bencode::from_bytes(bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    #[serde(rename = "piece length")]
    pub piece_length: u32,
    /// String <br>
    /// length := 20*n <br>
    /// Divide into length 20 substrings <br>
    /// Each string is the SHA1 hash of each piece <br>
    /// TODO: is it hex or base64 encoded -> hex
    pub pieces: ByteBuf,
    pub private: Option<i64>,
    pub name: String,
    #[serde(flatten)]
    pub mode: FileMode,
}

impl Info {
    pub fn get_pieces_as_sha1_hex(&self) -> Vec<String> {
        self.pieces
            .chunks(20)
            .map(util::encode_as_hex_string)
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileMode {
    SingleFile(SingleFile),
    MultipleFiles(MultipleFiles),
}

impl FileMode {
    pub fn unwrap_as_single(&self) -> &SingleFile {
        match self {
            FileMode::SingleFile(single_file) => single_file,
            FileMode::MultipleFiles(_) => panic!("FileMode was MultipleFiles"),
        }
    }

    pub fn unwrap_as_multi(&self) -> &MultipleFiles {
        match self {
            FileMode::SingleFile(_) => panic!("FileMode was SingleFile"),
            FileMode::MultipleFiles(multiple_files) => multiple_files,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleFile {
    pub length: u64,
    pub md5sum: Option<ByteBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipleFiles {
    pub files: Vec<File>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub length: u64,
    pub md5sum: Option<ByteBuf>,
    pub path: Vec<String>,
}
