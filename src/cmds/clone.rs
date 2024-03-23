use std::{
    collections::HashMap,
    env,
    io::{self, Write},
    mem,
    path::{Path, PathBuf},
};

use anyhow::Context;
use flate2::bufread::ZlibDecoder;
use tokio::runtime::Runtime;

use crate::{
    cmds,
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

pub fn clone(remote: &str, path: impl AsRef<Path>, mut _output: impl Write) -> anyhow::Result<()> {
    cmds::init::init(&path, io::sink())?;
    env::set_current_dir(path)?;

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
        let mut objects = HashMap::new();
        let mut delta_ref;
        let mut delta_offset_index;

        while pack[index..].len() > 20 {
            let object_type = pack[index] << 1 >> 5;
            let mut size = pack[index] as u64 & 0b0000_1111;
            let mut shift = 4;
            while pack[index] >= 128 {
                index += 1;
                size += (pack[index] as u64 & 0b0111_1111) << shift;
                shift += 7;
            }
            index += 1;

            (delta_ref, delta_offset_index) = (None, None);
            if object_type == OBJ_TYPE_OFFSET_DELTA {
                let mut offset = pack[index] as u64 & 0b0111_1111;
                let mut shift = 7;
                while pack[index] >= 128 {
                    index += 1;
                    offset += (pack[index] as u64 & 0b0111_1111) << shift;
                    shift += 7;
                }
                index += 1;
                delta_offset_index = Some(index - offset as usize);
            } else if object_type == OBJ_TYPE_REF_DELTA {
                delta_ref = Some(&pack[index..][..SHA_LEN]);
                index += SHA_LEN;
            }

            decompressor.reset(&pack[index..]);
            decompressed.clear();
            if io::copy(&mut decompressor, &mut decompressed).is_err() {
                break;
            }
            let out = decompressor.total_out();
            anyhow::ensure!(size == out, "decompressed data does not match object size");
            index += decompressor.total_in() as usize;

            match (delta_ref, delta_offset_index) {
                (None, None) => {
                    let r#type = match object_type {
                        1 => cmds::hash_object::Type::Commit,
                        2 => cmds::hash_object::Type::Tree,
                        3 => cmds::hash_object::Type::Blob,
                        4 => cmds::hash_object::Type::Tag,
                        _ => unreachable!("no other object types reachable"),
                    };

                    let mut hash = [0u8; SHA_LEN];
                    cmds::hash_object::hash_object(
                        true,
                        r#type,
                        cmds::hash_object::Source::Buf(&decompressed),
                        false,
                        hash.as_mut(),
                    )?;

                    objects.insert(hash, (mem::take(&mut decompressed), r#type));
                }

                (Some(_delta_ref), _) => {
                    continue;
                    // let Some(&(ref old_object, r#type)) = objects.get(delta_ref) else {
                    //     anyhow::bail!("failed to find reference in packfile")
                    // };

                    // let mut new_object = Vec::with_capacity(old_object.len());
                    // let mut delta_iter = decompressed.iter();

                    // // skip the size integers
                    // delta_iter
                    //     .by_ref()
                    //     .take_while(|&&byte| byte >= 128)
                    //     .for_each(|_| ());
                    // delta_iter
                    //     .by_ref()
                    //     .take_while(|&&byte| byte >= 128)
                    //     .for_each(|_| ());

                    // while let Some(&byte) = delta_iter.next() {
                    //     if byte < 128 {
                    //         // INSERT
                    //         let inserting = byte as usize & 0b0111_1111;
                    //         new_object.extend(delta_iter.by_ref().take(inserting));
                    //     } else {
                    //         // COPY
                    //         let _bytes_to_read = byte as usize & 0b0000_1111;
                    //     }
                    // }

                    // let mut hash = [0u8; SHA_LEN];
                    // cmds::hash_object::hash_object(
                    //     true,
                    //     r#type,
                    //     cmds::hash_object::Source::Buf(&new_object),
                    //     false,
                    //     hash.as_mut(),
                    // )?;

                    // objects.insert(hash, (new_object, r#type));
                }

                (_, Some(_delta_offset_index)) => {
                    continue;
                    // writeln!(output, "OFFSET INDEX {delta_offset_index}")?;
                }
            }
        }

        Ok(())
    })
}
