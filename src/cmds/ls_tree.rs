#[derive(clap::Args)]
pub struct Args {
    /// Recurse into subtrees
    #[arg(short)]
    pub recurse: bool,

    /// Show trees only
    #[arg(short = 'd')]
    pub trees_only: bool,

    /// Only display filenames
    #[arg(long)]
    pub name_only: bool,

    /// Abbreviate hashes
    #[arg(long)]
    pub abbrev: u8,
}

pub fn ls_tree(recurse: bool, trees_only: bool, name_only: bool, abbrev: u8) -> anyhow::Result<()> {
    //

    Ok(())
}
