use std::{io, path::Path};

use clap::{Parser, Subcommand};

#[cfg(test)]
use std::sync::Mutex;

mod cmds;
mod parsing;
mod utils;

const DOT_GIT: &str = ".git";
const OBJECTS: &str = "objects";
const REFS: &str = "refs";
const HEADS: &str = "heads";
const TAGS: &str = "tags";
const HEAD: &str = "HEAD";
const CONFIG: &str = "config";

const SHA_LEN: usize = 20;
const SHA_DISPLAY_LEN: usize = 40;

#[cfg(test)]
static FORCE_SINGLE_THREAD: Mutex<()> = Mutex::new(()); // used to synchronize unit tests

/// A simple clone of git
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Initialize an empty repository
    Init(cmds::init::Args),

    /// Clone a remote repository
    Clone(cmds::clone::Args),

    /// Create a commit in the repository
    Commit(cmds::commit::Args),

    /// Display the commit log
    Log(cmds::log::Args),

    /// Get and set configurations
    Config(cmds::config::Args),

    /// Print information about an object
    CatFile(cmds::cat_file::Args),

    /// Compute SHA hash of an object
    HashObject(cmds::hash_object::Args),

    /// List tree object contents
    LsTree(cmds::ls_tree::Args),

    /// Write a tree object to the .git database
    WriteTree(cmds::write_tree::Args),

    /// Create a commit object
    CommitTree(cmds::commit_tree::Args),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let stdout = io::stdout().lock();

    match cli.cmd {
        Cmd::Init(cmds::init::Args { path }) => {
            cmds::init::init(path.unwrap_or_else(|| ".".into()), stdout)
        }

        Cmd::CatFile(cmds::cat_file::Args { info, hash }) => {
            cmds::cat_file::cat_file(info.into(), &hash, stdout)
        }

        Cmd::HashObject(cmds::hash_object::Args {
            write,
            r#type,
            source,
        }) => cmds::hash_object::hash_object(write, r#type, source.into(), true, stdout),

        Cmd::LsTree(cmds::ls_tree::Args {
            recurse,
            trees_only,
            name_only,
            abbrev,
            hash,
        }) => cmds::ls_tree::ls_tree(recurse, trees_only, name_only, abbrev, &hash, stdout),

        Cmd::WriteTree(cmds::write_tree::Args {}) => cmds::write_tree::write_tree(stdout),

        Cmd::CommitTree(cmds::commit_tree::Args {
            parents,
            message,
            tree_hash,
        }) => cmds::commit_tree::commit_tree(&parents, &message, tree_hash.as_deref(), stdout),

        Cmd::Config(args) => cmds::config::config(args.into(), stdout),

        Cmd::Commit(cmds::commit::Args { message }) => cmds::commit::commit(message, stdout),

        Cmd::Log(cmds::log::Args { oneline, hash }) => {
            cmds::log::log(oneline, hash.as_deref(), stdout)
        }

        Cmd::Clone(cmds::clone::Args { remote, path }) => {
            cmds::clone::clone(&remote, path.as_deref().unwrap_or(Path::new(".")), stdout)
        }
    }
}
