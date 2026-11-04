#[cfg(test)]
mod TestINodeTable {
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
        let other_ino = t.lookup(other_path);
        assert!(t.path_of(other_ino) == Some(other_path));
    }

    #[test]
    fn test_multiple_lookup_single_ino() {
        let mut t = INodeTable::new(Path::new("root_path"));

        let other_path = Path::new("other_path");
        let first_other_ino = t.lookup(other_path);
        let second_other_ino = t.lookup(other_path);
        assert!(first_other_ino == second_other_ino);
    }

    #[test]
    fn test_multiple_lookup_multiple_ino() {
        let mut t = INodeTable::new(Path::new("root_path"));
        let mut prev_ino = ROOT;

        for i in 0..10 {
            let new_ino = t.lookup(format!("file_{i}:02"));
            assert!(new_ino != prev_ino);
            prev_ino = new_ino;
        }
    }

    #[test]
    fn test_forget() {
        let mut t = INodeTable::new(Path::new("root_path"));

        let other_path = Path::new("other_path");
        // nlookup 0->1
        let other_ino = t.lookup(other_path);
        // nlookup 1->2
        t.lookup(other_path);
        // nlookup 2->3
        t.lookup(other_path);

        // nlookup 3->1
        t.forget(other_ino, 2);
        assert!(t.path_of(other_ino).is_some());
        // nlookup 1->0
        // this should also drop ino from table
        t.forget(other_ino, 1);
        assert!(t.path_of(other_ino).is_none());
    }
}
