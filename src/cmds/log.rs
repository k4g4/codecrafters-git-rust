use std::{cmp, io::Write};

use anyhow::Context;
use chrono::{Local, TimeZone};

use crate::{
    cmds,
    parsing::{self, Commit},
    utils, SHA_DISPLAY_LEN,
};

#[derive(clap::Args)]
pub struct Args {
    /// Print one line for each commit
    #[arg(long)]
    pub oneline: bool,

    /// Commit log starting point
    pub hash: Option<String>,
}

pub fn log(oneline: bool, hash: Option<&str>, mut output: impl Write) -> anyhow::Result<()> {
    let hash = if let Some(hash) = hash {
        let hash = utils::find_object(hash.trim()).context("failed to find parent")?;
        let hash = hash.to_str().expect("path is utf-8").replace('/', "");
        hash[hash.len() - SHA_DISPLAY_LEN..].to_owned()
    } else {
        utils::get_head()?.context("no commits to display")?
    };

    fn get_commits(hash: &str, commits: &mut Vec<Commit>) -> anyhow::Result<()> {
        let mut contents = vec![];
        cmds::cat_file::cat_file(cmds::cat_file::Info::Print, hash, &mut contents)?;
        let (_, mut commit) = parsing::parse_commit(&contents)?;
        commit.hash = Some(hash.into());
        for parent in &commit.parents {
            get_commits(std::str::from_utf8(parent)?, commits)?;
        }
        commits.push(commit);

        Ok(())
    }

    let mut commits = vec![];
    get_commits(hash.trim(), &mut commits)?;
    commits.sort_by_key(|commit| cmp::Reverse(commit.timestamp));
    commits.dedup_by(|left, right| left.hash == right.hash);

    for Commit {
        hash,
        parents,
        author,
        timestamp,
        timezone,
        message,
    } in commits
    {
        if oneline {
            let message = message.replace('\n', " ");
            let message = if message.len() > 40 {
                format!("{}{}", &message[..37], "...")
            } else {
                message
            };
            writeln!(output, "{} {message}", &hash.unwrap()[..7])?;
        } else {
            writeln!(output, "commit {}", hash.unwrap())?;
            if parents.len() > 1 {
                write!(output, "Merge:\t")?;
                for parent in parents.iter().take(5) {
                    write!(output, "{} ", std::str::from_utf8(&parent[..7])?)?;
                }
                writeln!(output)?;
            }
            writeln!(output, "Author:\t{author}")?;
            // git log actually displays the date using this using the committer's timezone,
            // but this implementation uses the user's timezone instead.
            let datetime = Local
                .timestamp_opt(timestamp as i64, 0)
                .single()
                .context("failed to create datetime")?;
            writeln!(
                output,
                "Date:\t{} {}",
                datetime.format("%a %b %d  %H:%M:%S %Y"),
                std::str::from_utf8(&timezone)?
            )?;
            writeln!(output)?;
            let message = message.replace('\n', "\n\t");
            writeln!(output, "\t{}", message.trim())?;
            writeln!(output)?;
        }
    }

    Ok(())
}
