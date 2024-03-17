use clap::{Parser, Subcommand};

mod cmds;
mod parsing;
mod utils;

const DOT_GIT: &str = ".git";
const OBJECTS: &str = "objects";
const REFS: &str = "refs";
const HEAD: &str = "HEAD";

const SHA_LEN: usize = 20;
const SHA_DISPLAY_LEN: usize = 40;

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

    /// Print information about an object
    CatFile(cmds::cat_file::Args),

    /// Compute SHA hash of an object
    HashObject(cmds::hash_object::Args),

    /// List tree object contents
    LsTree(cmds::ls_tree::Args),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Init(cmds::init::Args { path }) => {
            cmds::init::init(path.unwrap_or_else(|| ".".into()))
        }

        Cmd::CatFile(cmds::cat_file::Args { info, hash }) => {
            cmds::cat_file::cat_file(info.into(), &hash, None)
        }

        Cmd::HashObject(cmds::hash_object::Args { write, source }) => {
            cmds::hash_object::hash_object(source.into(), write)
        }

        Cmd::LsTree(cmds::ls_tree::Args {
            recurse,
            trees_only,
            name_only,
            abbrev,
            hash,
        }) => cmds::ls_tree::ls_tree(recurse, trees_only, name_only, abbrev, &hash, None),
    }
}
