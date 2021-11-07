use bindings::windows::storage::streams::{DataWriter, IBuffer};

pub(crate) fn slice_to_buffer(bytes: &[u8]) -> windows::Result<IBuffer> {
    let writer = DataWriter::new()?;
    writer.write_bytes(bytes)?;
    let buffer = writer.detach_buffer()?;
    Ok(buffer)
}
