use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;
use tokio::runtime::Runtime;

use crate::parsing;

#[derive(clap::Args)]
pub struct Args {
    /// Remote repository
    pub remote: String,

    /// Repository path
    pub path: Option<PathBuf>,
}

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
        let (mut contents, refs) = parsing::http_response_body(service)(&contents)
            .context("invalid http response body")?;

        io::copy(&mut contents, &mut output)?;

        for (hash, name) in &refs {
            println!("{name}: {}", String::from_utf8_lossy(hash));
        }

        let response = client
            .post(format!("{remote}/{service}"))
            .body(format!(
                "0032want {}\n00000009done\n",
                std::str::from_utf8(&refs.first().unwrap().0).unwrap()
            ))
            .send()
            .await?;

        println!("{}", response.status());

        io::copy(&mut response.bytes().await?.as_ref(), &mut output)?;

        Ok(())
    })
}
