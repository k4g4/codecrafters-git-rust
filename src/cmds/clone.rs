use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;
use flate2::bufread::ZlibDecoder;
use tokio::runtime::Runtime;

use crate::{
    parsing::{self, pack_file_response},
    SHA_LEN,
};

const OBJ_TYPE_OFFSET_DELTA: u8 = 6;
const OBJ_TYPE_REF_DELTA: u8 = 7;

#[derive(clap::Args)]
pub struct Args {
    /// Remote repository
    pub remote: String,

    /// Repository path
    pub path: Option<PathBuf>,
}

enum ObjectMetadata {}

pub fn clone(remote: &str, _path: impl AsRef<Path>, mut output: impl Write) -> anyhow::Result<()> {
    Runtime::new()?.block_on(async {
        let remote = remote.trim_end_matches('/');
        let service = "git-upload-pack";

        let client = reqwest::Client::new();
        let response = client
            .get(format!("{remote}/info/refs?service={service}"))
            .send()
            .await?;
        anyhow::ensure!(
            [200, 304].contains(&response.status().as_u16()),
            "received {}",
            response.status()
        );

        let content_type = response
            .headers()
            .get("content-type")
            .expect("always has content-type")
            .to_str()
            .expect("should be utf-8");
        anyhow::ensure!(
            content_type == "application/x-git-upload-pack-advertisement",
            "received content-type: {content_type}"
        );

        let contents = response.bytes().await?;
        let (_, refs) = parsing::advertisement_response(service)(&contents)
            .context("invalid advertisement response body")?;

        let response = client
            .post(format!("{remote}/{service}"))
            .body({
                use std::fmt::Write;

                let mut body = String::new();
                for (hash, _) in refs {
                    writeln!(
                        body,
                        "0032want {}",
                        std::str::from_utf8(&hash).expect("hex-encoded")
                    )?;
                }
                writeln!(body, "00000009done")?;
                body
            })
            .send()
            .await?;
        anyhow::ensure!(
            response.status().is_success(),
            "received {}",
            response.status()
        );

        let contents = response.bytes().await?;
        let (pack, _) = pack_file_response(&contents).context("invalid pack file response body")?;

        let mut index = 12;
        let mut decompressor = ZlibDecoder::new(Default::default());
        let mut decompressed = vec![];
        let mut delta_ref: Option<[u8; SHA_LEN]> = None;
        let mut delta_offset = None;
        loop {
            let object_type = pack[index] << 1 >> 5;
            let mut size = pack[index] as u64 & 0b0000_1111;
            let mut shift = 4;
            while pack[index] >= 128 {
                index += 1;
                size += (pack[index] as u64 & 0b0111_1111) << shift;
                shift += 7;
            }
            index += 1;

            if object_type == OBJ_TYPE_OFFSET_DELTA {
                let mut offset = pack[index] as u64 & 0b0111_1111;
                let mut shift = 7;
                while pack[index] >= 128 {
                    index += 1;
                    offset += (pack[index] as u64 & 0b0111_1111) << shift;
                    shift += 7;
                }
                index += 1;
                delta_offset = Some(index - offset as usize);
            } else if object_type == OBJ_TYPE_REF_DELTA {
                delta_ref = Some(pack[index..SHA_LEN].try_into().expect("lengths match"));
                index += SHA_LEN;
            }

            decompressor.reset(&pack[index..]);
            decompressed.clear();
            if io::copy(&mut decompressor, &mut decompressed).is_err() {
                break;
            }
            let out = decompressor.total_out();
            anyhow::ensure!(size == out, "decompressed data does not match object size");
            let bytes_read = decompressor.total_in();
            writeln!(output, "compressed: {bytes_read}")?;
        }

        Ok(())
    })
}
