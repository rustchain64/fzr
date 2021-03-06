use anyhow::Error;
use async_std::fs;
use async_std::sync::Arc;
use libipld::{cid::Cid, Result};
use log::{error, info};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Instant;

use crate::data::content::{
    ContentItem, ContentItemBlock, ImageContent, ImageMetadata, TextContent, TextMetadata,
};
use crate::data::ipfs_client::IpfsClientRef;

pub async fn store_file(
    path: PathBuf,
    ipfs_client: IpfsClientRef,
) -> Result<Option<Cid>, Arc<Error>> {
    let start = Instant::now();

    let file_metadata = fs::metadata(&path).await.unwrap();
    let size_bytes = file_metadata.len();
    let buffer = fs::read(&path).await.unwrap(); // TODO: error handling

    if infer::is_image(&buffer[0..4]) {
        let (width_px, height_px) = image::image_dimensions(path).unwrap();
        let mime_type = infer::get(&buffer[0..4]).unwrap().mime_type().to_string();

        let buffer = buffer.into_boxed_slice();

        let block = ContentItemBlock {
            content: ContentItem::Image(
                ImageContent { buffer },
                ImageMetadata {
                    size_bytes,
                    mime_type,
                    width_px,
                    height_px,
                },
            ),
            size_bytes,
        };

        let ipfs_client = &ipfs_client.write().await;
        let cid = ipfs_client.add(&block).await?;

        info!(
            "Stored {:.2?}MB in {:.2?}.",
            size_bytes as f32 / 1_048_576_f32,
            start.elapsed()
        );

        Ok(Some(cid))
    } else {
        match String::from_utf8(buffer) {
            Ok(string) => {
                let block = ContentItemBlock {
                    content: ContentItem::Text(TextContent { string }, TextMetadata { size_bytes }),
                    size_bytes,
                };

                let ipfs_client = &ipfs_client.write().await;
                let cid = ipfs_client.add(&block).await?;

                info!(
                    "Stored {:.2?}MB in {:.2?}.",
                    size_bytes as f32 / 1_048_576_f32,
                    start.elapsed()
                );

                Ok(Some(cid))
            }
            Err(err) => {
                error!(
                    "Error decoding file as text (probably an unhandled binary file): {}",
                    err
                );
                Ok(None)
            }
        }
    }
}

pub async fn load_file(
    cid_string: String,
    ipfs_client: IpfsClientRef,
) -> Result<ContentItem, Arc<Error>> {
    let start = Instant::now();

    let ipfs_client = &ipfs_client.read().await;
    let cid = Cid::from_str(&cid_string).unwrap();
    let data = ipfs_client.get(&cid)?;

    info!(
        "Loaded {:.2?}MB in {:.2?}.",
        data.size_bytes as f32 / 1_048_576_f32,
        start.elapsed()
    );

    Ok(data.content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::ipfs_client::IpfsClient;

    use async_std::sync::RwLock;
    use tempfile::tempdir;

    use std::{error::Error, fs::File};
    use std::{io::Write, path::Path};

    /// Helper to create file in a directory and return full path.
    fn write_file<P>(dir: P, data: &[u8], file_name: &str) -> Result<PathBuf, Box<dyn Error>>
    where
        P: AsRef<Path>,
    {
        let path = dir.as_ref().join(file_name);
        let mut file = File::create(&path)?;
        file.write_all(data)?;
        Ok(path)
    }

    #[async_std::test]
    async fn test_store_load() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let client_ref = Arc::new(RwLock::new(IpfsClient::new().await.unwrap()));

        struct Test {
            name: &'static str,
            data: &'static [u8],
            file_name: &'static str,
            expected: ContentItem,
        }

        let tests = vec![
            Test {
                name: "round-trip smallest possible gif",
                data: b"GIF89a\x01\0\x01\0\0\0\0;",
                file_name: "smallest.gif",
                expected: ContentItem::Image(
                    ImageContent {
                        buffer: Box::new(*b"GIF89a\x01\0\x01\0\0\0\0;"),
                    },
                    ImageMetadata {
                        size_bytes: 14,
                        mime_type: "image/gif".into(),
                        width_px: 1,
                        height_px: 1,
                    },
                ),
            },
            Test {
                name: "round-trip text file",
                data: b"howdy",
                file_name: "howdy.txt",
                expected: ContentItem::Text(
                    TextContent {
                        string: "howdy".into(),
                    },
                    TextMetadata { size_bytes: 5 },
                ),
            },
        ];

        for test in tests.into_iter() {
            let client_ref = client_ref.clone();
            let path = write_file(dir.path(), test.data, test.file_name)?;
            let cid = store_file(path, client_ref.clone()).await.unwrap();
            let actual = load_file(cid.unwrap().to_string(), client_ref)
                .await
                .unwrap();

            assert_eq!(test.expected, actual, "{}", test.name);
        }
        Ok(())
    }
}