use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Read, Write};
use std::string;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Poki {
    pub segments: [Segment; 8],
    pub unresolved_table: Vec<String>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Segment {
    pub contents: Vec<u16>,
    pub relocation_table: Vec<RelocationTableEntry>,
    pub export_table: Vec<ExportTableEntry>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct RelocationTableEntry {
    pub offset: u16,
    pub segment_index: u16,
    pub segment_offset: u16,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ExportTableEntry {
    pub label: String,
    pub offset: u16,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct SegmentHeader {
    contents_size: u16,
    relocation_table_size: u16,
    export_table_size: u16,
}

impl SegmentHeader {
    fn deserialize(reader: &mut impl Read) -> Result<Self, PokiDeserializationError> {
        let contents_size = reader.read_word()?;
        let relocation_table_size = reader.read_word()?;
        let export_table_size = reader.read_word()?;

        Ok(SegmentHeader {
            contents_size,
            relocation_table_size,
            export_table_size,
        })
    }
}

impl Poki {
    pub fn new_empty() -> Self {
        Self {
            segments: [const {
                Segment {
                    contents: Vec::new(),
                    relocation_table: Vec::new(),
                    export_table: Vec::new(),
                }
            }; 8],
            unresolved_table: Vec::new(),
        }
    }

    pub fn serialize(&self, writer: &mut impl Write) -> Result<(), PokiSerializationError> {
        // Write the magic header.
        writer.write_all_words(&"poki".encode_utf16().collect::<Vec<_>>())?;

        // We being by first serializing the heading information for all of the segments, and then
        // continue by serializing each of the segments in turn.
        for segment in &self.segments {
            segment.serialize_header(writer)?;
        }
        for segment in &self.segments {
            segment.serialize(writer)?;
        }

        // Finally, we serialize the table of unresolved symbols.
        for symbol in &self.unresolved_table {
            let label_size = u16::try_from(symbol.encode_utf16().count()).map_err(|_| {
                PokiSerializationError::OversizedLabel(symbol.encode_utf16().count())
            })?;
            writer.write_word(label_size)?;
            writer.write_all_words(&symbol.encode_utf16().collect::<Vec<_>>())?;
        }

        Ok(())
    }

    pub fn deserialize(reader: &mut impl Read) -> Result<Self, PokiDeserializationError> {
        let mut magic_buffer = [0; 4];
        reader.read_exact_words(&mut magic_buffer)?;
        if magic_buffer != *"poki".encode_utf16().collect::<Vec<_>>() {
            return Err(PokiDeserializationError::InvalidMagic(magic_buffer));
        }

        let mut segment_headers = [SegmentHeader {
            contents_size: 0,
            relocation_table_size: 0,
            export_table_size: 0,
        }; 8];

        for segment_index in 0..8 {
            segment_headers[segment_index] = SegmentHeader::deserialize(reader)?;
        }

        let mut poki = Self::new_empty();

        for segment_index in 0..8 {
            poki.segments[segment_index] =
                Segment::deserialize(reader, segment_headers[segment_index])?;
        }

        let mut label_size = [0];
        loop {
            // HACK: This is kind of a goofy way to figure out if we've reached the (probable) end
            // of our reader, but whatever.
            if reader.read_words(&mut label_size)? == 0 {
                break;
            }

            let mut label = Vec::new();
            for _ in 0..label_size[0] {
                label.push(reader.read_word()?);
            }
            let label = String::from_utf16(&label)?;
            poki.unresolved_table.push(label);
        }

        Ok(poki)
    }
}

impl Segment {
    fn serialize_header(&self, writer: &mut impl Write) -> Result<(), PokiSerializationError> {
        let contents_size = u16::try_from(self.contents.len())
            .map_err(|_| PokiSerializationError::OversizedSegmentContents(self.contents.len()))?;
        writer.write_word(contents_size)?;

        let relocation_table_size =
            u16::try_from(3 * self.relocation_table.len()).map_err(|_| {
                PokiSerializationError::OversizedRelocationTable(3 * self.relocation_table.len())
            })?;
        writer.write_word(relocation_table_size)?;

        let export_table_size = u16::try_from(
            self.export_table
                .iter()
                .map(ExportTableEntry::len)
                .sum::<usize>(),
        )
        .map_err(|_| {
            PokiSerializationError::OversizedExportTable(
                self.export_table.iter().map(ExportTableEntry::len).sum(),
            )
        })?;
        writer.write_word(export_table_size)?;

        Ok(())
    }

    fn serialize(&self, writer: &mut impl Write) -> Result<(), PokiSerializationError> {
        writer.write_all_words(&self.contents)?;

        for relocation_table_entry in &self.relocation_table {
            relocation_table_entry.serialize(writer)?;
        }

        for export_table_entry in &self.export_table {
            export_table_entry.serialize(writer)?;
        }

        Ok(())
    }

    fn deserialize(
        reader: &mut impl Read,
        segment_header: SegmentHeader,
    ) -> Result<Self, PokiDeserializationError> {
        let mut contents = Vec::new();
        for _ in 0..segment_header.contents_size {
            contents.push(reader.read_word()?);
        }

        let mut relocation_table = Vec::new();
        if segment_header.relocation_table_size % 3 != 0 {
            return Err(PokiDeserializationError::InvalidRelocationTableSize(
                segment_header.relocation_table_size,
            ));
        } else {
            for _ in 0..segment_header.relocation_table_size / 3 {
                let offset = reader.read_word()?;
                let segment_index = reader.read_word()?;
                let segment_offset = reader.read_word()?;

                relocation_table.push(RelocationTableEntry {
                    offset,
                    segment_index,
                    segment_offset,
                });
            }
        }

        let mut export_table = Vec::new();
        let mut remaining_export_table_size = segment_header.export_table_size;
        while remaining_export_table_size != 0 {
            let label_size = reader.read_word()?;
            if label_size + 2 > remaining_export_table_size {
                return Err(PokiDeserializationError::StringOverrun(label_size + 1 - remaining_export_table_size));
            }

            let mut label = Vec::new();
            for _ in 0..label_size {
                label.push(reader.read_word()?);
            }
            let label = String::from_utf16(&label)?;

            let offset = reader.read_word()?;

            export_table.push(ExportTableEntry {
                 label,
                 offset,
             });

            remaining_export_table_size -= label_size + 2;
        }

        Ok(Self {
            contents,
            relocation_table,
            export_table,
        })
    }
}

impl RelocationTableEntry {
    fn serialize(&self, writer: &mut impl Write) -> Result<(), PokiSerializationError> {
        writer.write_all_words(&[self.offset, self.segment_index, self.segment_offset])?;
        Ok(())
    }
}

impl ExportTableEntry {
    fn serialize(&self, writer: &mut impl Write) -> Result<(), PokiSerializationError> {
        let label_size = u16::try_from(self.label.encode_utf16().count()).map_err(|_| {
            PokiSerializationError::OversizedLabel(self.label.encode_utf16().count())
        })?;
        writer.write_word(label_size)?;
        writer.write_all_words(&self.label.encode_utf16().collect::<Vec<_>>())?;
        writer.write_word(self.offset)?;

        Ok(())
    }

    fn len(&self) -> usize {
        1 + self.label.encode_utf16().count() + 1
    }
}

#[derive(Debug)]
pub enum PokiSerializationError {
    IOError(io::Error),
    OversizedSegmentContents(usize),
    OversizedRelocationTable(usize),
    OversizedExportTable(usize),
    OversizedLabel(usize),
}

impl Display for PokiSerializationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Self::IOError(e) => write!(f, "{}", e),
            Self::OversizedSegmentContents(s) => {
                write!(
                    f,
                    "unable to serialize poki with segment of length {s}, above the limit of 65536"
                )
            }
            Self::OversizedRelocationTable(s) => {
                write!(f, "unable to serialize poki with relocation table of length {s}, above the limit of 65536")
            }
            Self::OversizedExportTable(s) => {
                write!(f, "unable to serialize poki with export table of length {s}, above the limit of 65536")
            }
            Self::OversizedLabel(s) => {
                write!(
                    f,
                    "unable to serialize poki with exported label of {s}, above the limit of 65536"
                )
            }
        }
    }
}

impl Error for PokiSerializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::IOError(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

impl From<io::Error> for PokiSerializationError {
    fn from(value: io::Error) -> Self {
        Self::IOError(value)
    }
}

#[derive(Debug)]
pub enum PokiDeserializationError {
    IOError(io::Error),
    FromUtf16Error(string::FromUtf16Error),
    InvalidMagic([u16; 4]),
    InvalidRelocationTableSize(u16),
    StringOverrun(u16),
}

impl Display for PokiDeserializationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Self::IOError(e) => write!(f, "{}", e),
            Self::FromUtf16Error(e) => write!(f, "{}", e),
            Self::InvalidMagic(m) => write!(
                f,
                "expected file to begin with magic words \"poki\", found {:?} instead",
                m
            ),
            Self::InvalidRelocationTableSize(s) => write!(f, "file claims to contain a relocation table of size {s}, but relocation table sizes must be divible by 3"),
            Self::StringOverrun(n) => write!(f, "export table contains string whose claimed length overruns the export table by {n} words)")
        }
    }
}

impl Error for PokiDeserializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IOError(e) => Some(e),
            Self::FromUtf16Error(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for PokiDeserializationError {
    fn from(value: io::Error) -> Self {
        Self::IOError(value)
    }
}

impl From<string::FromUtf16Error> for PokiDeserializationError {
    fn from(value: string::FromUtf16Error) -> Self {
        Self::FromUtf16Error(value)
    }
}

trait ReadWordsExt {
    fn read_word(&mut self) -> io::Result<u16>;
    fn read_words(&mut self, buffer: &mut [u16]) -> io::Result<usize>;
    fn read_exact_words(&mut self, buffer: &mut [u16]) -> io::Result<()>;
}

impl<T> ReadWordsExt for T
where
    T: Read,
{
    fn read_word(&mut self) -> io::Result<u16> {
        let mut buffer = [0; 2];

        self.read_exact(&mut buffer)?;
        Ok(u16::from_ne_bytes(buffer))
    }

    fn read_words(&mut self, buffer: &mut [u16]) -> io::Result<usize> {
        let len = buffer.len().checked_mul(2).unwrap();
        let ptr: *mut u8 = buffer.as_mut_ptr().cast();
        let buffer = unsafe { std::slice::from_raw_parts_mut(ptr, len) };

        self.read(buffer).map(|n| n / 2)
    }

    fn read_exact_words(&mut self, buffer: &mut [u16]) -> io::Result<()> {
        let len = buffer.len().checked_mul(2).unwrap();
        let ptr: *mut u8 = buffer.as_mut_ptr().cast();
        let buffer = unsafe { std::slice::from_raw_parts_mut(ptr, len) };

        self.read_exact(buffer)
    }
}

trait WriteWordsExt {
    fn write_word(&mut self, word: u16) -> io::Result<()>;
    fn write_all_words(&mut self, words: &[u16]) -> io::Result<()>;
}

impl<T> WriteWordsExt for T
where
    T: Write,
{
    fn write_word(&mut self, word: u16) -> io::Result<()> {
        self.write_all(&word.to_ne_bytes())
    }

    fn write_all_words(&mut self, words: &[u16]) -> io::Result<()> {
        let len = words.len().checked_mul(2).unwrap();
        let ptr: *const u8 = words.as_ptr().cast();
        let words = unsafe { std::slice::from_raw_parts(ptr, len) };

        self.write_all(words)
    }
}
