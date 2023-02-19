use crate::prelude::*;

pub struct MigrateFileInfo {
    modified: SystemTime,
    scan: u64,
}

impl MigrateFileInfo {
    pub fn new(modified: SystemTime, scan: u64) -> Self {
        Self { modified, scan }
    }
}

pub struct MigrateFolderInfo {
    scan: u64,
}

impl MigrateFolderInfo {
    pub fn new(scan: u64) -> Self {
        Self { scan }
    }
}

#[derive(Default)]
pub struct RenderCache {
    _hash: u64,
}

#[derive(Default)]
pub struct Updates {
    pub files: AHashSet<PathBuf>,
    pub folders: AHashSet<PathBuf>,
}

impl Updates {
    pub fn clear(&mut self) {
        self.files.clear();
        self.folders.clear();
    }
}

#[derive(Default)]
pub struct Inner {
    ready: bool,
    migrate_files: AHashMap<PathBuf, MigrateFileInfo>,
    migrate_folders: AHashMap<PathBuf, MigrateFolderInfo>,
    scan: u64,
    // render: AHashMap<PathBuf, RenderCache>,
    updates: Updates,
}

// impl Inner {
//     pub fn new() -> Self {
//         Self {
//             cache: Default::default(),
//             updates: Default::default(),
//         }
//     }
// }

#[derive(Default, Clone)]
pub struct Sink {
    inner: Arc<Mutex<Inner>>,
}

impl Sink {
    // pub fn new() -> Self {
    //     Self {
    //         inner: Arc::new(Mutex::new(Inner::default())),
    //     }
    // }

    pub fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    pub fn begin(&self) {
        self.inner().updates.clear();
    }

    pub fn finish(&self) {}

    pub fn init_state(&self) -> bool {
        let mut inner = self.inner();
        if !inner.ready {
            inner.ready = true;
            true
        } else {
            false
        }
    }

    pub async fn init(&self, ctx: &Arc<Context>) -> Result<()> {
        if self.init_state() {
            log_info!("Clean", "cleaning up target...");
            ctx.clean().await?;
            ctx.ensure_folders().await?;
        }
        Ok(())
    }

    pub fn migrate(&self, ctx: &Context, list: &[PathBuf]) -> Result<()> {
        let mut inner = self.inner();
        let mut copy_files = Vec::new();
        let mut create_folders = AHashSet::new();
        // let scan = inner;
        inner.scan += 1;
        let scan = inner.scan;

        for file in list.iter() {
            let from = ctx.src_folder.join(file);
            let modified = std::fs::metadata(&from)?.modified()?;
            match inner.migrate_files.get_mut(file) {
                Some(entry) => {
                    entry.scan = scan;
                    if entry.modified != modified {
                        entry.modified = modified;
                        copy_files.push(file);
                    }
                }
                None => {
                    inner
                        .migrate_files
                        .insert(file.to_owned(), MigrateFileInfo::new(modified, scan));
                    copy_files.push(file);
                }
            };

            if let Some(folder) = file.parent() {
                if folder.to_string_lossy().len() != 0 {
                    match inner.migrate_folders.get_mut(folder) {
                        Some(entry) => {
                            entry.scan = scan;
                        }
                        None => {
                            inner
                                .migrate_folders
                                .insert(folder.to_owned(), MigrateFolderInfo::new(scan));
                            create_folders.insert(folder.to_path_buf());
                        }
                    }
                }
            }
        }

        // remove missing files
        inner.migrate_files.retain(|k, v| {
            if v.scan != scan {
                log_info!("Remove", "file: {}", k.display());
                std::fs::remove_file(ctx.site_folder.join(k)).unwrap_or_else(|err| {
                    log_error!("Unable to remove file `{}`: {err}", k.display());
                });
                false
            } else {
                true
            }
        });

        // remove missing folders
        for (folder, f) in inner.migrate_folders.iter() {
            if f.scan != inner.scan {
                log_info!("Remove", "folder: {}", folder.display());
                std::fs::remove_dir_all(ctx.site_folder.join(folder)).ok();
            }
        }

        // create new folders
        for folder in create_folders {
            log_trace!(
                "Migrate",
                "{} `{}`",
                style("file:").cyan(),
                folder.display()
            );
            std::fs::create_dir_all(ctx.site_folder.join(folder))?;
        }

        // copy new files
        for file in copy_files.iter() {
            let to_file = ctx.site_folder.join(file);
            log_trace!(
                "Migrate",
                "{} `{}` to `{}`",
                style("file:").cyan(),
                file.display(),
                to_file.display()
            );
            std::fs::copy(ctx.src_folder.join(file), to_file)?;
        }

        Ok(())
    }
}
