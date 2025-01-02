use std::{io::SeekFrom, path::Path};

use sha1::{Digest, Sha1};
use tokio::{
    fs::File,
    io::{AsyncSeekExt, AsyncWriteExt},
};

use crate::util::{self, create_buffer};

const CHUNK_SIZE_POW2: usize = 14;
const CHUNK_SIZE: usize = 1 << CHUNK_SIZE_POW2;

#[derive(Clone, Copy, Debug)]
pub enum PieceResult {
    Success,
    NotComplete,
    IntegrityFault,
}

pub struct PieceChunk {
    pub begin: usize,
    pub length: usize,
}

#[derive(Default, Clone, Copy)]
pub struct PieceState {
    started: bool,
    completed: bool,
}

pub struct TorrentFile {
    file_length: u64,
    piece_count: usize,
    piece_length: usize,
    // end_piece_length: usize,
    // --
    file: File,
    file_state: Box<[PieceState]>,
    piece_buffers: Box<[Option<PieceBuffer>]>,
}

impl TorrentFile {
    pub async fn new(
        path: &str,
        file_length: u64,
        piece_count: usize,
        piece_length: usize,
    ) -> anyhow::Result<Self> {
        let path = sanitize_filename::sanitize_with_options(
            path,
            sanitize_filename::Options {
                windows: true,
                truncate: true,
                replacement: "_",
            },
        );

        let file_exists = Path::new(&path).exists();
        let file = if file_exists {
            File::options().write(true).open(&path).await?
        } else {
            let mut file = File::create(&path).await?;
            file.seek(SeekFrom::Start(file_length - 1)).await?;
            file.write_all(&[0]).await?;
            file
        };

        Ok(Self {
            file_length,
            piece_count,
            piece_length,
            file,
            file_state: (0..piece_count).map(|_| PieceState::default()).collect(),
            piece_buffers: (0..piece_count).map(|_| None).collect(),
            // piece_buffers: (0..piece_count).map(|_| PieceBuffer::new(piece_length)).collect(),
            // piece_buffers: (0..piece_count - 1)
            //     .map(|_| PieceBuffer::new(piece_length))
            //     .chain(std::iter::once(PieceBuffer::new(
            //         end_piece_length,
            //     )))
            //     .collect(),
        })
    }

    fn end_piece_length(&self) -> usize {
        (self.file_length - ((self.piece_count - 1) as u64 * self.piece_length as u64)) as usize
    }

    fn get_piece_length(&self, index: usize) -> usize {
        if index != self.piece_count - 1 { self.piece_length } else { self.end_piece_length() }
    }

    fn get_cursor_pos(&self, index: usize, begin: usize) -> u64 {
        (index as u64 * self.piece_length as u64) + begin as u64
    }

    pub fn next_piece_chunk(&mut self, index: usize) -> Option<PieceChunk> {
        if !self.file_state[index].started {
            let piece_length = self.get_piece_length(index);
            self.piece_buffers[index] = Some(PieceBuffer::new(piece_length));
            self.file_state[index].started = true;
            self.file_state[index].completed = false;
        }

        let piece = self.piece_buffers[index].as_ref().unwrap();
        piece.next_chunk()
    }

    pub fn write_piece_chunk(&mut self, index: usize, begin: usize, chunk: &[u8]) {
        if !self.file_state[index].started {
            let piece_length = self.get_piece_length(index);
            self.piece_buffers[index] = Some(PieceBuffer::new(piece_length));
            self.file_state[index].started = true;
            self.file_state[index].completed = false;
        }

        let piece = self.piece_buffers[index].as_mut().unwrap();
        piece.write_chunk(begin, chunk);
    }

    pub async fn flush_piece(&mut self, index: usize) -> anyhow::Result<()> {
        // if !self.file_state[index].completed {
        //     bail!("Piece not yet completed.");
        // }

        self.file
            .seek(SeekFrom::Start(self.get_cursor_pos(index, 0)))
            .await?;

        let piece = &self.piece_buffers[index].as_ref().unwrap().piece;
        self.file.write_all(piece).await?;

        self.file_state[index].completed = true;
        let _ = self.piece_buffers[index].take();

        Ok(())
    }

    pub fn reset_piece(&mut self, index: usize) {
        self.file_state[index].started = false;
        self.file_state[index].completed = false;
        if let Some(p) = self.piece_buffers[index].as_mut() {
            p.reset();
        }
    }

    pub fn check_piece(&self, index: usize, piece_hash: &str) -> PieceResult {
        if self.file_state[index].completed {
            return PieceResult::Success;
        }
        if !self.file_state[index].started {
            return PieceResult::NotComplete;
        }

        self.piece_buffers[index].as_ref().unwrap().check_piece(piece_hash)
    }
}

pub struct ChunkState {
    begin: usize,
    is_filled: bool,
}

impl ChunkState {
    pub fn new_empty(begin: usize) -> Self {
        Self {
            begin,
            is_filled: false,
        }
    }
}

pub struct PieceBuffer {
    piece: Box<[u8]>,
    chunk_length: usize,
    chunks: Box<[ChunkState]>,
}

impl PieceBuffer {
    fn new(piece_length: usize) -> Self {
        let mut chunk_count = piece_length >> CHUNK_SIZE_POW2;
        let mut chunk_length = CHUNK_SIZE;

        if chunk_count == 0 {
            chunk_count = 1;
            chunk_length = piece_length;
        }

        Self {
            piece: create_buffer(piece_length),
            chunk_length,
            chunks: (0..chunk_count)
                .map(|ci| ChunkState::new_empty(ci * chunk_length))
                .collect(),
        }
    }

    fn next_chunk(&self) -> Option<PieceChunk> {
        let Some(chunk_state) = self.chunks.iter().find(|cs| !cs.is_filled) else {
            return None;
        };
        return Some(PieceChunk {
            begin: chunk_state.begin,
            length: self.chunk_length, // TODO: chunk_length chunk-by-chunk ???
        })
    }

    fn write_chunk(&mut self, begin: usize, chunk: &[u8]) {
        let Some(chunk_state) = self.chunks.iter_mut().find(|cs| cs.begin == begin) else {
            return;
        };

        let end = begin + chunk.len();
        let piece_buffer_slice = &mut self.piece[begin..end];
        piece_buffer_slice.copy_from_slice(chunk);

        chunk_state.is_filled = true;
    }

    fn reset(&mut self) {
        self.chunks.iter_mut().for_each(|cs| cs.is_filled = false);
    }

    fn check_piece(&self, piece_hash: &str) -> PieceResult {
        let all_filled = self.chunks.iter().all(|cs| {
            // println!("chunk.is_filled: {}", cs.is_filled);
            cs.is_filled
        });
        if !all_filled {
            return PieceResult::NotComplete;
        }

        let mut sha1_hasher = Sha1::new();
        sha1_hasher.update(&self.piece);
        let this_piece_hash: [u8; 20] = sha1_hasher.finalize().into();
        let this_piece_hash = util::encode_as_hex_string(&this_piece_hash);

        // println!("expected: {}\nresult:   {}", piece_hash, &this_piece_hash);

        if this_piece_hash == piece_hash {
            PieceResult::Success
        } else {
            PieceResult::IntegrityFault
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io::SeekFrom, path::Path};

    use sha1::{Digest, Sha1};
    use tokio::{fs::File, io::{AsyncReadExt, AsyncSeekExt}};

    use crate::util;

    use super::{PieceResult, TorrentFile, CHUNK_SIZE};

    #[test]
    fn copy_file_random() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let path = "res/Godel.pdf";
            let torrent_path = "res/Godel-torrent.pdf";

            let file_length = Path::new(path).metadata().unwrap().len();
            let piece_length = 2 * CHUNK_SIZE;
            let piece_count = (file_length / piece_length as u64) as usize;
            let piece_count = if file_length % piece_length as u64 == 0 {
                piece_count
            } else {
                piece_count + 1
            };

            let mut torrent_file = TorrentFile::new(torrent_path, file_length, piece_count, piece_length).await.unwrap();
            
            let mut file = File::open(path).await.unwrap();
            
            let mut file_piece_buffer = util::create_buffer(piece_length);
            let mut piece_index = 0;
            while piece_index < piece_count {
                println!("Writing piece {piece_index}");
                loop {
                    let Some(next_chunk) = torrent_file.next_piece_chunk(piece_index) else {
                        break;
                    };

                    let cursor = torrent_file.get_cursor_pos(piece_index, next_chunk.begin);
                    // println!("Chunk request: {} -- {} -- {} -> {}", next_chunk.begin, next_chunk.length, next_chunk.begin + next_chunk.length, cursor);

                    file.seek(SeekFrom::Start(cursor)).await.unwrap();

                    let file_piece_buffer = &mut file_piece_buffer[next_chunk.begin .. next_chunk.begin + next_chunk.length];
                    file.read_exact(file_piece_buffer).await.unwrap();

                    torrent_file.write_piece_chunk(piece_index, next_chunk.begin, file_piece_buffer);
                    // let result = torrent_file.check_piece(piece_index, "asd");
                    // println!("Current result: {:?}", result);
                }

                // let b1 = &torrent_file.piece_buffers[piece_index].as_ref().unwrap().piece;
                // let b2 = &file_piece_buffer;
                // if b1.len() != b2.len() {
                //     println!("DIFFERENT LENGTH {}, {}", b1.len(), b2.len());
                // }
                // else {
                //     for i in 0..b1.len() {
                //         if b1[i] != b2[i] {
                //             println!("DIFFERENT BYTES {i}");
                //             break;
                //         }
                //     }
                // }
                
                
                let mut sha1_hasher = Sha1::new();
                sha1_hasher.update(&file_piece_buffer[0..torrent_file.get_piece_length(piece_index)]);
                let piece_hash: [u8; 20] = sha1_hasher.finalize().into();
                let piece_hash = util::encode_as_hex_string(&piece_hash);

                let result = torrent_file.check_piece(piece_index, &piece_hash);
                dbg!(result);
                match result {
                    PieceResult::Success => {
                        torrent_file.flush_piece(piece_index).await.unwrap();
                        piece_index += 1;
                    },
                    PieceResult::NotComplete => {},
                    PieceResult::IntegrityFault => {
                        torrent_file.reset_piece(piece_index);
                    },
                }
            }
        });
    }

}
