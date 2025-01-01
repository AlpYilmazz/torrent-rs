use std::{io::SeekFrom, path::Path};

use tokio::{
    fs::File,
    io::{AsyncSeekExt, AsyncWriteExt},
};

const CHUNK_SIZE_POW2: u32 = 14;
const CHUNK_SIZE: u32 = 1 << CHUNK_SIZE_POW2;

pub struct TorrentFile {
    path: String,
    file: File,
    state: FileState,
}

impl TorrentFile {
    pub async fn new(
        path: &str,
        piece_count: u32,
        piece_length: u32,
        end_piece_length: u32,
    ) -> anyhow::Result<Self> {
        let path = sanitize_filename::sanitize_with_options(
            path,
            sanitize_filename::Options { windows: true, truncate: true, replacement: "_" }
        );

        let file_exists = Path::new(&path).exists();
        let file = if file_exists {
            File::open(&path).await?
        } else {
            let total_len =
                ((piece_count - 1) as u64 * piece_length as u64) + end_piece_length as u64;
            let mut file = File::create(&path).await?;
            file.seek(SeekFrom::Start(total_len - 1)).await?;
            file.write_all(&[0]).await?;
            file
        };
        
        Ok(Self {
            path,
            file,
            state: FileState::new(piece_count, piece_length, end_piece_length),
        })
    }

    pub async fn write_piece(
        &mut self,
        index: u32,
        begin: u32,
        piece: &[u8],
    ) -> anyhow::Result<()> {
        self.file
            .seek(SeekFrom::Start(self.state.cursor_pos(index, begin)))
            .await?;
        self.file.write_all(piece).await?;

        self.state.update_chunk(index, begin, piece.len() as u32);

        Ok(())
    }
}

pub struct FileState {
    piece_count: u32,
    piece_length: u32,
    end_piece_length: u32,
    pieces: Vec<FilePiece>,
}

impl FileState {
    pub fn new(piece_count: u32, piece_length: u32, end_piece_length: u32) -> Self {
        Self {
            piece_count,
            piece_length,
            end_piece_length,
            pieces: (0..piece_count - 1)
                .map(|i| FilePiece::new(i, piece_length))
                .chain(std::iter::once(FilePiece::new(
                    piece_count - 1,
                    end_piece_length,
                )))
                .collect(),
        }
    }

    pub fn cursor_pos(&self, index: u32, begin: u32) -> u64 {
        (index as u64 * self.piece_length as u64) + begin as u64
    }

    pub fn next_chunk(&self, index: u32) -> Option<PieceChunk> {
        let piece = self.pieces.get(index as usize)?;
        let chunk = piece.chunks.iter().find(|chunk| !chunk.is_filled)?;
        Some(PieceChunk {
            begin: chunk.begin,
            length: piece.chunk_length,
        })
    }

    pub fn update_chunk(&mut self, index: u32, begin: u32, length: u32) -> bool {
        let Some(piece) = self.pieces.get_mut(index as usize) else {
            return false;
        };
        if piece.chunk_length != length {
            return false;
        }

        let chunk = piece.chunks.iter_mut().find(|chunk| chunk.begin == begin);

        if let Some(chunk) = chunk {
            chunk.is_filled = true;
            return true;
        }
        return false;
    }
}

pub struct FilePiece {
    _index: u32,
    chunk_length: u32,
    chunks: Vec<ChunkState>,
}

impl FilePiece {
    fn new(index: u32, piece_length: u32) -> Self {
        let mut chunk_count = piece_length >> CHUNK_SIZE_POW2;
        let mut chunk_length = CHUNK_SIZE;

        if chunk_count == 0 {
            chunk_count = 1;
            chunk_length = piece_length;
        }

        Self {
            _index: index,
            chunk_length,
            chunks: (0..chunk_count)
                .map(|ci| ChunkState::new_empty(ci * chunk_length))
                .collect(),
        }
    }
}

pub struct ChunkState {
    begin: u32,
    is_filled: bool,
}

impl ChunkState {
    pub fn new_empty(begin: u32) -> Self {
        Self {
            begin,
            is_filled: false,
        }
    }
}

pub struct PieceChunk {
    pub begin: u32,
    pub length: u32,
}

impl PieceChunk {
    pub fn end(&self) -> u32 {
        self.begin as u32 + self.length
    }
}
