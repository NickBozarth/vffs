use std::{collections::HashMap, path::{Path, PathBuf}};

use fuser::INodeNo;


// Root node of any FUSE fs must be INodeNo(1)
pub const ROOT: INodeNo = INodeNo(1);


// Data about ino
pub struct INodeEntry {
    path: PathBuf,
    // nlookup can be thought of as a reference count
    // when forget is called and nlookup == 0, it drops entry from table
    nlookup: u64
}

pub struct INodeTable {
    by_ino: HashMap<INodeNo, INodeEntry>,
    by_path: HashMap<PathBuf, INodeNo>,
    next_ino: u64
}

impl INodeTable {
    // Create a table pre-initialized with a root node
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        let mut t = Self {
            by_ino: HashMap::new(),
            by_path: HashMap::new(),
            next_ino: 2
        };

        t.by_path.insert(root.as_ref().to_path_buf(), ROOT);
        t.by_ino.insert(ROOT, INodeEntry {
            path: root.as_ref().to_path_buf(),
            nlookup: 1
        });

        t
    }

    pub fn path_of(&self, ino: INodeNo) -> Option<&Path> {
        self.by_ino
            .get(&ino)
            .map(|e| e.path.as_path())
    }

    // Looks up corresponding ino from a path
    //  if ino is registered => increment nlookup and return ino
    //  else => create new INodeEntry with nlookup at 1 and return new ino
    pub fn get_or_create_ino<P: AsRef<Path>>(&mut self, path: P) -> INodeNo {
        match self.by_path.get(path.as_ref()) {
            Some(ino) => {
                if let Some(entry) = self.by_ino.get_mut(ino) {
                    entry.nlookup += 1;
                }

                *ino
            },
            None => {
                let ino = INodeNo(self.next_ino);
                self.next_ino += 1;
                self.by_path.insert(
                    path.as_ref().to_path_buf(),
                    ino
                );

                self.by_ino.insert(ino, INodeEntry {
                    path: path.as_ref().to_path_buf(),
                    nlookup: 1
                });

                ino
            }
        }
    }


    // Removes entries if their nlookup <= n
    // Ignores ROOT
    pub fn forget(&mut self, ino: INodeNo, n: u64) {
        if ino == ROOT {
            return;
        }

        let Some(entry) = self.by_ino.get_mut(&ino) else {
            return;
        };

        if entry.nlookup > n {
            entry.nlookup -= n;
        } else {
            self.by_path.remove(&entry.path);
            self.by_ino.remove(&ino);
        }
    }

    pub fn by_path<P: AsRef<Path>>(&self, path: P) -> Option<INodeNo> {
        self.by_path.get(path.as_ref()).copied()
    }
}
