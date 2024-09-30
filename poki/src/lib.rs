use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Read, Write};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Poki<'a> {
    pub segments: [Segment<'a>; 8],
    pub unresolved_table: Vec<&'a str>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Segment<'a> {
    pub contents: Vec<u16>,
    pub relocation_table: Vec<RelocationTableEntry>,
    pub export_table: Vec<ExportTableEntry<'a>>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct RelocationTableEntry {
    pub offset: u16,
    pub segment_index: u16,
    pub segment_offset: u16,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct ExportTableEntry<'a> {
    pub label: &'a str,
    pub offset: u16,
}

impl Poki<'_> {
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
            writer.write(&label_size.to_ne_bytes())?;
            writer.write_all_words(&symbol.encode_utf16().collect::<Vec<_>>())?;
        }

        Ok(())
    }

    pub fn deserialize(reader: &impl Read) -> Result<Self, PokiDeserializationError> {
        todo!()
    }
}

impl Segment<'_> {
    fn serialize_header(&self, writer: &mut impl Write) -> Result<(), PokiSerializationError> {
        let contents_size = u16::try_from(self.contents.len())
            .map_err(|_| PokiSerializationError::OversizedSegmentContents(self.contents.len()))?;
        writer.write(&contents_size.to_ne_bytes())?;

        let relocation_table_size =
            u16::try_from(3 * self.relocation_table.len()).map_err(|_| {
                PokiSerializationError::OversizedRelocationTable(3 * self.relocation_table.len())
            })?;
        writer.write(&relocation_table_size.to_ne_bytes())?;

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
        writer.write(&export_table_size.to_ne_bytes())?;

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
}

impl RelocationTableEntry {
    fn serialize(&self, writer: &mut impl Write) -> Result<(), PokiSerializationError> {
        writer.write_all_words(&[self.offset, self.segment_index, self.segment_offset])?;
        Ok(())
    }
}

impl ExportTableEntry<'_> {
    fn serialize(&self, writer: &mut impl Write) -> Result<(), PokiSerializationError> {
        let label_size = u16::try_from(self.label.encode_utf16().count()).map_err(|_| {
            PokiSerializationError::OversizedLabel(self.label.encode_utf16().count())
        })?;
        writer.write(&label_size.to_ne_bytes())?;
        writer.write_all_words(&self.label.encode_utf16().collect::<Vec<_>>())?;
        writer.write(&self.offset.to_ne_bytes())?;

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
}

impl Display for PokiDeserializationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Self::IOError(e) => write!(f, "{}", e),
        }
    }
}

impl Error for PokiDeserializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::IOError(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

trait WriteWords {
    fn write_all_words(&mut self, buf: &[u16]) -> io::Result<()>;
}

impl<T> WriteWords for T
where
    T: Write,
{
    fn write_all_words(&mut self, buf: &[u16]) -> io::Result<()> {
        // We have to to a bit of byte-mucking to interpret our array of words as a byte array so
        // that it can be written to the writer, but this saves a lot of conversions elsewhere.
        let len = buf.len().checked_mul(2).unwrap();
        let ptr: *const u8 = buf.as_ptr().cast();
        let buf = unsafe { std::slice::from_raw_parts(ptr, len) };
        self.write_all(buf)
    }
}
