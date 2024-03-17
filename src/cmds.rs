pub mod cat_file;
pub mod hash_object;
pub mod init;
pub mod ls_tree;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        path::PathBuf,
        sync::{Mutex, MutexGuard},
    };

    const TEST_DIR: &'static str = "test_dir";
    static FORCE_SINGLE_THREAD: Mutex<()> = Mutex::new(());

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

        init::init(".").unwrap();

        let mut filenames = fs::read_dir(".git")
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        filenames.sort();
        assert_eq!(filenames, ["HEAD", "objects", "refs"]);

        assert_eq!(
            fs::read(PathBuf::from(".git").join("HEAD")).unwrap(),
            b"ref: refs/heads/main\n"
        );
    }

    #[test]
    fn store_and_load() {
        let _setup = Setup::init();
        let file = "a.txt";

        init::init(".").unwrap();

        fs::write(&file, "Hello, world").unwrap();

        hash_object::hash_object(hash_object::Source::Path(PathBuf::from(file)), true).unwrap();

        assert!(fs::metadata(
            PathBuf::from(".git")
                .join("objects")
                .join("db")
                .join("e9dba55ea8fd4d5be3868b015e044be0848ec5")
        )
        .unwrap()
        .is_file());

        let mut output = vec![];
        cat_file::cat_file(
            cat_file::Info::Print,
            "dbe9dba55ea8fd4d5be3868b015e044be0848ec5",
            Some(&mut output),
        )
        .unwrap();
        assert_eq!(output, b"Hello, world");

        output.clear();
        cat_file::cat_file(
            cat_file::Info::Type,
            "dbe9dba55ea8fd4d5be3868b015e044be0848ec5",
            Some(&mut output),
        )
        .unwrap();
        assert_eq!(output, b"blob");

        output.clear();
        cat_file::cat_file(
            cat_file::Info::Size,
            "dbe9dba55ea8fd4d5be3868b015e044be0848ec5",
            Some(&mut output),
        )
        .unwrap();
        assert_eq!(output, b"12");
    }
}
