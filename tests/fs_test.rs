#[cfg(test)]
mod test_inode_table {
    use std::path::Path;

    use vffs::inode_table::{INodeTable, ROOT};

    #[test]
    fn test_path_of_root() {
        let root_path = Path::new("root_path");
        let t = INodeTable::new(root_path);
   assert!(t.path_of(ROOT) == Some(root_path))
    }

    #[test]
    fn test_lookup() {
        let mut t = INodeTable::new(Path::new("root_path"));

        let other_path = Path::new("other_path");
        let other_ino = t.get_or_create_ino(other_path);
        assert!(t.path_of(other_ino) == Some(other_path));
    }

    #[test]
    fn test_multiple_lookup_single_ino() {
        let mut t = INodeTable::new(Path::new("root_path"));

        let other_path = Path::new("other_path");
        let first_other_ino = t.get_or_create_ino(other_path);
        let second_other_ino = t.get_or_create_ino(other_path);
        assert!(first_other_ino == second_other_ino);
    }

    #[test]
    fn test_multiple_lookup_multiple_ino() {
        let mut t = INodeTable::new(Path::new("root_path"));
        let mut prev_ino = ROOT;

        for i in 0..10 {
            let new_ino = t.get_or_create_ino(format!("file_{i}:02"));
            assert!(new_ino != prev_ino);
            prev_ino = new_ino;
        }
    }

    #[test]
    fn test_forget() {
        let mut t = INodeTable::new(Path::new("root_path"));

        let other_path = Path::new("other_path");
        // nlookup 0->1
        let other_ino = t.get_or_create_ino(other_path);
        // nlookup 1->2
        t.get_or_create_ino(other_path);
        // nlookup 2->3
        t.get_or_create_ino(other_path);

        // nlookup 3->1
        t.forget(other_ino, 2);
        assert!(t.path_of(other_ino).is_some());
        // nlookup 1->0
        // this should also drop ino from table
        t.forget(other_ino, 1);
        assert!(t.path_of(other_ino).is_none());
    }
}

#[cfg(test)]
mod test_handle_manager {
    use std::{fs, sync::Arc};

    use tempfile::tempdir;
    use vffs::handle_manager::HandleManager;


    #[test]
    fn test_get_file() {
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();
        let f = fs::File::open(temp_path).unwrap();
        let mut hm = HandleManager::new();

        let handle = hm.register_handle(f, temp_path);
        let first = hm.get_file(handle).unwrap();
        let second = hm.get_file(handle).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn test_path_of() {
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();
        let f = fs::File::open(temp_path).unwrap();
        let mut hm = HandleManager::new();

        let handle = hm.register_handle(f, temp_path);
        assert!(hm.path_of(handle) == Some(temp_path.to_path_buf()));
    }

    #[test]
    fn test_release() {
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();
        let f = fs::File::open(temp_path).unwrap();
        let mut hm = HandleManager::new();

        let handle = hm.register_handle(f, temp_path);
        assert!(hm.path_of(handle) == Some(temp_path.to_path_buf()));
        hm.release(handle);
        assert!(hm.path_of(handle).is_none());
    }
}

#[cfg(test)]
mod test_fs {
    use std::{fs, io::ErrorKind, os::unix::fs::PermissionsExt, path::{Path, PathBuf}, time::{Duration, UNIX_EPOCH}};

    use fuser::BackgroundSession;
    use tempfile::{TempDir, tempdir};
    use vffs::fs::fault_fs::FaultInjectionFileSystem;

    struct TempFs {
        _session: BackgroundSession,
        src: TempDir,
        mnt: TempDir
    }

    impl TempFs {
        pub fn new() -> Self {
            let src = tempdir().unwrap();
            let mnt = tempdir().unwrap();

            let session = FaultInjectionFileSystem::new(
                src.path(),
                mnt.path(),
                Duration::from_secs(1)
            )
                .spawn_mount()
                .unwrap();

            Self { src, mnt, _session: session }
        }

        pub fn join_src<P: AsRef<Path>>(&self, path: P) -> PathBuf {
            self.src.path().join(path)
        }

        pub fn join_mnt<P: AsRef<Path>>(&self, path: P) -> PathBuf {
            self.src.path().join(path)
        }
    }


    fn list_dir<P: AsRef<Path>>(dir: P) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .map(|osstring| osstring.into_string().unwrap())
            .collect();
        v.sort();
        v
    }


    #[test]
    fn ls_src_matches_mnt() {
        let tfs = TempFs::new();
        fs::write(tfs.join_src("a"), b"").unwrap();
        fs::write(tfs.join_src("b"), b"").unwrap();
        fs::create_dir(tfs.join_src("c")).unwrap();
        assert!(list_dir(tfs.src.path()) == vec!["a", "b", "c"]);
        assert!(list_dir(tfs.mnt.path()) == vec!["a", "b", "c"]);
    }

    #[test]
    fn ls_dir_in_dir_matches_mnt() {
        let tfs = TempFs::new();
        fs::create_dir_all(tfs.join_src("a/b"))
            .unwrap();
        fs::create_dir(tfs.join_src("a/c"))
            .unwrap();

        assert!(list_dir(tfs.join_src("a")) == vec!["b", "c"]);
        assert!(list_dir(tfs.join_src("a")) == vec!["b", "c"]);
    }

    #[test]
    fn test_mkdir_in_mnt_() {
        let tfs = TempFs::new();
        fs::create_dir(tfs.join_mnt("a")).unwrap();
        fs::create_dir(tfs.join_mnt("b")).unwrap();
        fs::create_dir(tfs.join_mnt("c")).unwrap();

        assert!(list_dir(tfs.src.path()) == vec!["a", "b", "c"]);
        assert!(list_dir(tfs.mnt.path()) == vec!["a", "b", "c"]);
    }

    #[test]
    fn test_meta_is_tracked() {
        let tfs = TempFs::new();

        for i in 0..10 {
            let path = tfs.join_src(format!("f{i}"));
            fs::write(&path, vec![b'a'; i]).unwrap();

            let meta = fs::symlink_metadata(path).unwrap();
            assert!(meta.is_file());
            assert!(meta.len() == i as u64);
        }
    }

    #[test]
    fn test_dir_on_file() {
        let tfs = TempFs::new();
        fs::write(tfs.join_mnt("a"), b"").unwrap();
        assert!(fs::read_dir(tfs.join_mnt("a")).is_err());
    }

    #[test]
    fn test_not_found() {
        let tfs = TempFs::new();
        let error = fs::metadata(tfs.join_mnt("not_a_file")).unwrap_err();
        assert!(error.kind() == ErrorKind::NotFound);
    }

    #[test]
    fn test_change_perms() {
        let tfs = TempFs::new();
        fs::write(tfs.join_mnt("a"), b"").unwrap();
        let old_perms = fs::metadata(tfs.join_mnt("a"))
            .unwrap()
            .permissions()
            .mode();

        fs::set_permissions(
            tfs.join_mnt("a"),
            fs::Permissions::from_mode(0o111)
        )
            .unwrap();

        let new_perms = fs::metadata(tfs.join_mnt("a"))
            .unwrap()
            .permissions()
            .mode();

        assert!(old_perms != new_perms);
        assert!(new_perms & 0o777 == 0o111);
    }

    #[test]
    fn test_read_write() {
        let tfs = TempFs::new();
        fs::write(tfs.join_mnt("a"), b"test").unwrap();
        let read = fs::read(tfs.join_mnt("a"))
            .unwrap();

        assert!(read.as_slice() == b"test");
    }

    #[test]
    fn test_set_modified() {
        let tfs = TempFs::new();
        fs::write(tfs.join_mnt("a"), b"").unwrap();
        let new_time = UNIX_EPOCH + Duration::from_secs(100);
        fs::OpenOptions::new()
            .write(true)
            .open(tfs.join_mnt("a"))
            .unwrap()
            .set_modified(new_time)
            .unwrap();

        let modified = fs::metadata(tfs.join_mnt("a"))
            .unwrap()
            .modified()
            .unwrap();

        assert!(new_time == modified);
    }

}
