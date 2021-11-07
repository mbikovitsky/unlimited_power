use windows::{
    runtime::Result,
    Storage::Streams::{DataWriter, IBuffer},
};

pub(crate) fn slice_to_buffer(bytes: &[u8]) -> Result<IBuffer> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(bytes)?;
    let buffer = writer.DetachBuffer()?;
    Ok(buffer)
}
