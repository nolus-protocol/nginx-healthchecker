use anyhow::Result;

use tokio::io::AsyncWriteExt;

use crate::service::ServiceOutputWriter;

#[repr(transparent)]
pub(super) struct UpstreamSectionEntry<T>(T);

impl<T> UpstreamSectionEntry<T> {
    #[inline]
    pub const fn new(buffer: T) -> Self {
        Self(buffer)
    }
}

impl<T> UpstreamSectionEntry<T>
where
    T: AsyncWriteExt + Unpin,
{
    async fn write_terminated(buf: &mut T, output: &str) -> Result<()> {
        buf.write_all(b"    ").await?;

        buf.write_all(output.as_bytes()).await?;

        buf.write_all(b";\n").await.map_err(From::from)
    }
}

impl<T> ServiceOutputWriter for UpstreamSectionEntry<T>
where
    T: AsyncWriteExt + Unpin,
{
    async fn write_out_prepended<'r>(
        &'r mut self,
        output: &'r str,
    ) -> Result<()> {
        Self::write_terminated(&mut self.0, output).await
    }

    async fn write_out_entry<'r>(&'r mut self, output: &'r str) -> Result<()> {
        Self::write_terminated(&mut self.0, output).await
    }
}
