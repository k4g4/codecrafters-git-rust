pub mod cat_file;
pub mod clone;
pub mod commit;
pub mod commit_tree;
pub mod config;
pub mod hash_object;
pub mod init;
pub mod log;
pub mod ls_tree;
pub mod write_tree;

#[cfg(test)]
mod tests {
    use crate::FORCE_SINGLE_THREAD;

    use super::*;
    use std::{env, fs, io, path::PathBuf, sync::MutexGuard};

    const TEST_DIR: &'static str = "test_dir";

    struct Setup(MutexGuard<'static, ()>);
    impl Setup {
        fn init() -> Self {
            let guard = FORCE_SINGLE_THREAD.lock().unwrap();
            let _ = fs::remove_dir_all(TEST_DIR);
            fs::create_dir(TEST_DIR).unwrap();
            env::set_current_dir(TEST_DIR).unwrap();
            Self(guard)
        }
    }
    impl Drop for Setup {
        fn drop(&mut self) {
            env::set_current_dir("..").unwrap();
            let _ = fs::remove_dir_all(TEST_DIR);
        }
    }

    #[test]
    fn initialize() {
        let _setup = Setup::init();

        init::init(".", io::sink()).unwrap();

        let mut filenames = fs::read_dir(".git")
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        filenames.sort();
        assert_eq!(filenames, ["HEAD", "config", "objects", "refs"]);

        assert_eq!(
            fs::read(PathBuf::from(".git").join("HEAD")).unwrap(),
            b"ref: refs/heads/main\n"
        );
    }

    #[test]
    fn store_and_load() {
        let _setup = Setup::init();
        let file = "a.txt";
        const HASH: &str = "dbe9dba55ea8fd4d5be3868b015e044be0848ec5";

        init::init(".", io::sink()).unwrap();

        fs::write(&file, "Hello, world").unwrap();

        let mut output = vec![];
        hash_object::hash_object(
            true,
            hash_object::Type::Blob,
            hash_object::Source::Path(PathBuf::from(file)),
            true,
            &mut output,
        )
        .unwrap();

        assert_eq!(output, b"dbe9dba55ea8fd4d5be3868b015e044be0848ec5\n");

        assert!(fs::metadata(
            PathBuf::from(".git")
                .join("objects")
                .join(&HASH[..2])
                .join(&HASH[2..])
        )
        .unwrap()
        .is_file());

        let mut output = vec![];
        cat_file::cat_file(cat_file::Info::Print, HASH, &mut output).unwrap();
        assert_eq!(output, b"Hello, world");

        output.clear();
        cat_file::cat_file(cat_file::Info::Type, HASH, &mut output).unwrap();
        assert_eq!(output, b"blob");

        output.clear();
        cat_file::cat_file(cat_file::Info::Size, HASH, &mut output).unwrap();
        assert_eq!(output, b"12");
    }

    #[test]
    fn store_and_load_tree() {
        let _setup = Setup::init();

        init::init(".", io::sink()).unwrap();

        fs::write("test_file_1.txt", "hello world").unwrap();
        fs::create_dir("test_dir_1").unwrap();
        fs::write("test_dir_1/test_file_2.txt", "hello world").unwrap();
        fs::create_dir("test_dir_2").unwrap();
        fs::write("test_dir_2/test_file_3.txt", "hello world").unwrap();

        let mut output = vec![];
        write_tree::write_tree(&mut output).unwrap();

        assert_eq!(output, b"1d6753fb1a4263946e82a7ce64b7dcaa3191dfb2\n");

        output.clear();
        ls_tree::ls_tree(true, false, false, 8, "1d675", &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\
040000\ttree\tafe59fc8f3bee47e\ttest_dir_1
100644\tblob\t95d09f2b10159347\ttest_file_2.txt
040000\ttree\t56280867b54d00c9\ttest_dir_2
100644\tblob\t95d09f2b10159347\ttest_file_3.txt
100644\tblob\t95d09f2b10159347\ttest_file_1.txt
"
        );
    }
}
