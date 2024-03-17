const DOT_GIT: &str = ".git";
const OBJECTS: &str = "objects";
const REFS: &str = "refs";
const HEAD: &str = "HEAD";

const SHA_LEN: usize = 40;

mod init;
pub use init::*;

mod cat_file;
pub use cat_file::*;

mod hash_object;
pub use hash_object::*;
