use anyhow::Result;
use clap::{Parser, Subcommand};

mod cmds;

/// A simple clone of git
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Subcommands,
}

#[derive(Subcommand)]
enum Subcommands {
    /// Initialize an empty repository
    Init(cmds::init::Args),

    /// Print information about an object
    CatFile(cmds::cat_file::Args),

    /// Compute SHA hash of an object
    HashObject(cmds::hash_object::Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Subcommands::Init(cmds::init::Args { path }) => {
            cmds::init::init(path.unwrap_or_else(|| ".".into()))
        }

        Subcommands::CatFile(cmds::cat_file::Args { info, hash }) => {
            cmds::cat_file::cat_file(info.into(), &hash, None)
        }

        Subcommands::HashObject(cmds::hash_object::Args { write, source }) => {
            cmds::hash_object::hash_object(source.into(), write)
        }
    }
}
