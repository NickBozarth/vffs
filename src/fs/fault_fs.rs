use std::{ffi::OsStr, io, path::{Path, PathBuf}, time::{Duration, SystemTime}};

use fuser::{BackgroundSession, BsdFileFlags, Config, Errno, FileHandle, Filesystem, FopenFlags, Generation, INodeNo, LockOwner, OpenFlags, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, Request, TimeOrNow};
use indexmap::IndexMap;

use crate::{fs::{errors::MapperResult, mapper::{Mapper, SetAttrArgs}}, rules::{context::{ActionCtx, ConditionCallback, PostactionCallback, PostconditionCallback, PreactionCallback, PreconditionCallback, Reply}, errors::RuleManagerResult, manager::RuleManager}};
use crate::rules::default_callbacks::*;

pub struct FaultInjectionFileSystem {
    mountpoint: PathBuf,
    ttl: Duration,
    fs_mapper: Mapper,
    rule_manager: RuleManager
}

impl FaultInjectionFileSystem {
    pub fn new<S, M, R>(
     source_dir: S,
        mountpoint: M,
        rules: R,
        timeout: Duration
    ) -> RuleManagerResult<Self>
    where
        S: AsRef<Path>,
        M: AsRef<Path>,
        R: AsRef<Path>
    {
        Ok(Self {
            mountpoint: mountpoint.as_ref().to_path_buf(),
            ttl: timeout,
            fs_mapper: Mapper::new(source_dir.as_ref()),
            rule_manager: RuleManager::new(rules.as_ref())?
        })
    }

    pub fn spawn_mount(self) -> io::Result<BackgroundSession> {
        if let Err(errors) = self.rule_manager.validate_callbacks() {
            let errors = errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            return Err(io::Error::other(format!("Invalid rules file:\n{errors}")));
        }

        let mut options = Config::default();
        options.mount_options.extend_from_slice(&[
            fuser::MountOption::RW,
        ]);

        let mountpoint = self.mountpoint.clone();
        fuser::spawn_mount(self, &mountpoint, &options)
    }

    fn act_with_rules<T>(
        &self,
        action_ctx: ActionCtx,
        action: impl FnOnce() -> MapperResult<T>,
        reply_constructor: impl FnOnce(&mut T) -> Reply
    ) -> Result<T, Errno> {
        let action_type = action_ctx.action_type();
        let mut ctx = action_ctx.into_ctx();

        for e in self.rule_manager.call_preactions(&mut ctx) {
            eprintln!("{action_type:?} post-action callback failed: {e}");
        }

        let mut res = action()
            .map_err(|e| e.errno());

        let reply = match &mut res {
            Ok(r) => reply_constructor(r),
            Err(e) => Reply::Err(*e)
        };
        let mut ctx = ctx.attach_reply(reply);

        for e in self.rule_manager.call_postactions(&mut ctx) {
            eprintln!("{action_type:?} post-action callback failed: {e}");
        }

        // Callbacks will mostly just modify the Reply it is given, which essentially holds a &mut res
        // BUT any Reply may be converted into a Reply::Err
        // SO Reply::Err is an owned Errno and must be checked
        match ctx.reply() {
            Reply::Err(e) => Err(*e),
            _ => res
        }
    }


    pub fn with_precondition(
        mut self,
        cb_name: &str,
        cb: PreconditionCallback
    ) -> Self {
        self.rule_manager.add_precondition(cb_name, cb);
        self
    }

    pub fn with_postcondition(
        mut self,
        cb_name: &str,
        cb: PostconditionCallback
    ) -> Self {
        self.rule_manager.add_postcondition(cb_name, cb);
        self
    }

    pub fn with_preaction(
        mut self,
        cb_name: &str,
        cb: PreactionCallback
    ) -> Self {
        self.rule_manager.add_preaction(cb_name, cb);
        self
    }

    pub fn with_postaction(
        mut self,
        cb_name: &str,
        cb: PostactionCallback
    ) -> Self {
     self.rule_manager.add_postaction(cb_name, cb);
        self
    }

    pub fn with_default_conditions(mut self) -> Self {
        self.rule_manager.add_precondition("match_action", match_action);
        self.rule_manager.add_postcondition("match_action", match_action);
        self.rule_manager.add_precondition("probability", probability);
        self.rule_manager.add_postcondition("probability", probability);
        self.rule_manager.add_precondition("match_pid", match_pid);
        self.rule_manager.add_postcondition("match_pid", match_pid);
        self.rule_manager.add_precondition("match_gid", match_gid);
        self.rule_manager.add_postcondition("match_gid", match_gid);
        self.rule_manager.add_precondition("match_uid", match_uid);
        self.rule_manager.add_postcondition("match_uid", match_uid);

        self.rule_manager.add_postcondition("is_err", is_err);
        self.rule_manager.add_postcondition("err_matches", err_matches);
        self.rule_manager.add_postcondition("duration_over", duration_over);

        self
    }

    pub fn with_default_actions(mut self) -> Self {
        self.rule_manager.add_preaction("delay_by", delay_by);
        self.rule_manager.add_postaction("delay_by", delay_by);
        self.rule_manager.add_postaction("delay_by_pct", delay_by_pct);
        self.rule_manager.add_postaction("map_errno", map_errno);
        self.rule_manager.add_postaction("fail_with_code", fail_with_code);
        self.rule_manager.add_postaction("corrupt_read", corrupt_read);
        self.rule_manager.add_preaction("log_action_ctx", log_action_ctx);
        self.rule_manager.add_postaction("log_action_ctx", log_action_ctx);
        self.rule_manager.add_preaction("log_action_type", log_action_type);
        self.rule_manager.add_postaction("log_action_type", log_action_type);
        self.rule_manager.add_postaction("log_reply", log_reply);
        self.rule_manager.add_preaction("log_ctx", log_ctx);
        self.rule_manager.add_postaction("log_ctx", log_ctx);
        self.rule_manager.add_postaction("log_read_as_string", log_read_as_string);


        self
    }
}

impl Filesystem for FaultInjectionFileSystem {
    fn lookup(&self,
        req: &Request,
        parent: INodeNo,
        name: &OsStr,
        reply: ReplyEntry
    ) {
        match self.act_with_rules(
            ActionCtx::Lookup { req, parent, name },
            || self.fs_mapper.lookup_attr(parent, name),
            |r| Reply::Lookup(r)
        ) {
            Ok(attr) => reply.entry(&self.ttl, &attr, Generation(0)),
            Err(e) => reply.error(e),
        }
    }

    fn forget(&self, _req: &Request, ino: INodeNo, nlookup: u64) {
        let _ = self.fs_mapper.forget(ino, nlookup);
    }

    fn getattr(
        &self,
        req: &Request,
        ino: INodeNo,
        fh: Option<FileHandle>,
        reply: ReplyAttr
    ) {
        match self.act_with_rules(
            ActionCtx::Getattr { req, ino, fh },
            || self.fs_mapper.getattr(ino, fh),
            |r| Reply::Getattr(r)
        ) {
            Ok(attr) => reply.attr(&self.ttl, &attr),
            Err(e) => reply.error(e),
        }
    }

    fn setattr(
        &self,
        req: &Request,
        ino: INodeNo,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        ctime: Option<SystemTime>,
        fh: Option<FileHandle>,
        crtime: Option<SystemTime>,
        chgtime: Option<SystemTime>,
        bkuptime: Option<SystemTime>,
        flags: Option<BsdFileFlags>,
        reply: ReplyAttr,
    )
    {
        let args = SetAttrArgs { mode, uid, gid, size, atime, mtime, fh };

        match self.act_with_rules(
            ActionCtx::Setattr {
                req, ino, mode, uid, gid, size, atime, mtime,
                ctime, fh, crtime, chgtime, bkuptime, flags
            },
            || self.fs_mapper.setattr(ino, args),
            |r| Reply::Setattr(r)
        ) {
            Ok(attr) => reply.attr(&self.ttl, &attr),
            Err(e) => reply.error(e),
        }
    }

    fn readdir(
        &self,
        req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    )
    {
        match self.act_with_rules(
            ActionCtx::Readdir { req, ino, fh, offset },
            || self.fs_mapper.list_dir(ino),
            |r| Reply::Readdir(r)
        ) {
            Ok(entries) => {
                let entries_iter = entries
                    .into_iter()
                    .enumerate()
                    .skip(offset as usize);
                for (i, (ino, kind, name)) in entries_iter {
                    if reply.add(ino, (i + 1) as u64, kind, name) {
                        break;
                    }
                }

                reply.ok();
            }
            Err(e) => reply.error(e)
        }
    }

    fn open(
        &self,
        req: &Request,
        ino: INodeNo,
        flags: OpenFlags,
        reply: ReplyOpen
    ) {
        match self.act_with_rules(
            ActionCtx::Open { req, ino, flags },
            || self.fs_mapper.open(ino, flags),
            |r| Reply::Open(r)
        ) {
            Ok(fh) => reply.opened(fh, FopenFlags::empty()),
            Err(e) => reply.error(e)
        }
    }

    fn read(
        &self,
        req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        size: u32,
        flags: OpenFlags,
        lock_owner: Option<LockOwner>,
        reply: ReplyData,
    )
    {
        match self.act_with_rules(
            ActionCtx::Read { req, ino, fh, offset, size, flags, lock_owner },
            || self.fs_mapper.read(fh, offset, size),
            |r| Reply::Read(r)
        ) {
            Ok(data) => reply.data(&data),
            Err(e) => reply.error(e),
        }
    }

    fn write(
        &self,
        req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        data: &[u8],
        write_flags: fuser::WriteFlags,
        flags: OpenFlags,
        lock_owner: Option<fuser::LockOwner>,
        reply: fuser::ReplyWrite,
    )
    {
        match self.act_with_rules(
            ActionCtx::Write {
                req, ino, fh, offset, data,
                write_flags, flags, lock_owner
            },
            || self.fs_mapper.write(fh, offset, data),
            |r| Reply::Write(r)
        ) {
            Ok(n) => reply.written(n),
            Err(e) => reply.error(e),
        }
    }

    fn release(
        &self,
        req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        flags: OpenFlags,
        lock_owner: Option<fuser::LockOwner>,
        flush: bool,
        reply: fuser::ReplyEmpty,
    )
    {
        match self.act_with_rules(
            ActionCtx::Release { req, ino, fh, flags, lock_owner, flush },
            || self.fs_mapper.release(fh),
            |_| Reply::Release
        ) {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e)
        }
    }

    fn fsync(
        &self,
        req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        datasync: bool,
        reply: ReplyEmpty,
    )
    {
        match self.act_with_rules(
            ActionCtx::Fsync { req, ino, fh, datasync },
            || self.fs_mapper.fsync(fh, datasync),
            |_| Reply::Fsync
        ) {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e)
        }
    }
}
