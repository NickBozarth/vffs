use std::{ffi::{OsStr, OsString}, marker::PhantomData, time::{Duration, Instant, SystemTime}};

use fuser::{BsdFileFlags, Errno, FileAttr, FileHandle, FileType, INodeNo, LockOwner, OpenFlags, Request, TimeOrNow, WriteFlags};

use crate::rules::{errors::ModResult, gen_fn::GenArgs, rules::ActionType};


pub type ActionCallback<T> = fn(GenArgs, &mut Context<T>) -> ModResult<()>;
pub type PreactionCallback = ActionCallback<CtxPreAction>;
pub type PostactionCallback = ActionCallback<CtxPostAction>;

pub type ConditionCallback<T> = fn(GenArgs, &Context<T>) -> ModResult<bool>;
pub type PreconditionCallback = ConditionCallback<CtxPreAction>;
pub type PostconditionCallback = ConditionCallback<CtxPostAction>;


pub enum ActionCtx<'a> {
    Lookup {
        req: &'a Request,
        parent: INodeNo,
        name: &'a OsStr
    },
    Forget {
        req: &'a Request,
        ino: INodeNo,
        nlookup: u64
    },
    Getattr {
        req: &'a Request,
        ino: INodeNo,
        fh: Option<FileHandle>
    },
    Setattr {
        req: &'a Request,
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
    },
    Readdir {
        req: &'a Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64
    },
    Open {
        req: &'a Request,
        ino: INodeNo,
        flags: OpenFlags
    },
    Read {
        req: &'a Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        size: u32,
        flags: OpenFlags,
        lock_owner: Option<LockOwner>

    },
    Write {
        req: &'a Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        data: &'a [u8],
        write_flags: WriteFlags,
        flags: OpenFlags,
        lock_owner: Option<LockOwner>
    },
    Release {
        req: &'a Request,
        ino: INodeNo,
        fh: FileHandle,
        flags: OpenFlags,
        lock_owner: Option<LockOwner>,
        flush: bool
    },
    Fsync {
        req: &'a Request,
        ino: INodeNo,
        fh: FileHandle,
        datasync: bool,
    }

}

impl<'a> ActionCtx<'a> {
    pub fn into_ctx(self) -> Context<'a, CtxPreAction> {
        Context {
            pre_action_time: Instant::now(),
            post_action_time: None,

            action_ctx: self,
            reply: None,
            _ty: PhantomData
        }
    }

    pub fn req(&self) -> &'a Request {
        match self {
            Self::Lookup {  req, .. }
            | Self::Forget {req, .. }
            | Self::Getattr {req, .. }
            | Self::Setattr {req, .. }
            | Self::Readdir {req, .. }
            | Self::Open {req, .. }
            | Self::Read {req, .. }
            | Self::Write {req, .. }
            | Self::Release {req, .. }
            | Self::Fsync {req, .. } => *req
        }
    }

    pub fn ino(&self) -> Option<INodeNo> {
        match self  {
            Self::Forget { ino, .. }
            | Self::Getattr { ino, .. }
            | Self::Setattr { ino, .. }
            | Self::Readdir { ino, .. }
            | Self::Open { ino, .. }
            | Self::Read { ino, .. }
            | Self::Write { ino, .. }
            | Self::Release { ino, .. }
            | Self::Fsync { ino, .. } => Some(*ino),

            Self::Lookup { .. } => None
        }
    }

    pub fn fh(&self) -> Option<FileHandle> {
        match self {
            Self::Readdir { fh, .. }
            | Self::Read { fh, .. }
            | Self::Write { fh, .. }
            | Self::Release { fh, .. }
            | Self::Fsync { fh, .. } => Some(*fh),

            Self::Getattr { fh, .. }
            | Self::Setattr { fh, .. } => *fh,

            Self::Lookup { .. }
            | Self::Forget { .. }
            | Self::Open { .. } => None
        }
    }

    pub fn action_type(&self) -> ActionType {
        match self {
            Self::Lookup { .. } => ActionType::Lookup,
            Self::Forget { .. } => ActionType::Forget,
            Self::Getattr { .. } => ActionType::Getattr,
            Self::Setattr { .. } => ActionType::Setattr,
            Self::Readdir { .. } => ActionType::Readdir,
            Self::Open { .. } => ActionType::Open,
            Self::Read { .. } => ActionType::Read,
            Self::Write { .. } => ActionType::Write,
            Self::Release { .. } => ActionType::Release,
            Self::Fsync { .. } => ActionType::Fsync
        }
    }
}


pub enum Reply<'a> {
    Err(Errno),

    Lookup(&'a mut FileAttr),
    Forget,
    Getattr(&'a mut FileAttr),
    Setattr(&'a mut FileAttr),
    Readdir(&'a mut Vec<(INodeNo, FileType, OsString)>),
    Open(&'a mut FileHandle),
    Read(&'a mut [u8]),
    Write(&'a mut u32),
    Release,
    Fsync,
}

impl Reply<'_> {
    pub fn action_type(&self) -> ActionType {
        match self {
            Self::Err(_) => ActionType::None,
            Self::Lookup(_) => ActionType::Lookup,
            Self::Forget => ActionType::Forget,
            Self::Getattr(_) => ActionType::Getattr,
            Self::Setattr(_) => ActionType::Setattr,
            Self::Readdir(_) => ActionType::Readdir,
            Self::Open(_) => ActionType::Open,
            Self::Read(_) => ActionType::Read,
            Self::Write(_) => ActionType::Write,
            Self::Release => ActionType::Release,
            Self::Fsync => ActionType::Fsync
        }
    }
}


pub struct CtxPreAction;
pub struct CtxPostAction;

// Two Contexts will be sent per fs action
// - Once before action occurs
// - Once after action occurs
// reply and post_action_time will always be None if this is a pre-action Context
// reply and post_action_time will always be Some(_) if this is a post-action Context
pub struct Context<'a, T> {
    pre_action_time: Instant,
    post_action_time: Option<Instant>,

    action_ctx: ActionCtx<'a>,
    reply: Option<Reply<'a>>,
    _ty: PhantomData<T>
}

impl<'a, T> Context<'a, T> {
    pub fn action_ctx(&self) -> &ActionCtx<'a> {
        &self.action_ctx
    }

    pub fn action_type(&self) -> ActionType {
        self.action_ctx.action_type()
    }
}

impl<'a> Context<'a, CtxPreAction> {
    pub fn attach_reply(self, reply: Reply<'a>) -> Context<'a, CtxPostAction> {
        Context {
            pre_action_time: self.pre_action_time,
            post_action_time: Some(Instant::now()),

            action_ctx: self.action_ctx,
            reply: Some(reply),
            _ty: PhantomData
        }
    }
}

impl<'a> Context<'a, CtxPostAction> {
    pub fn reply(&mut self) -> &mut Reply<'a> {
        self.reply
            .as_mut()
            .expect("Context<CtxPostAction> will always contain a reply")
    }

    pub fn pre_action_time(&self) -> Instant {
        self.pre_action_time
    }

    pub fn post_action_time(&self) -> Instant {
        self.post_action_time
            .expect("Context<'_, CtxPostAction> will always contain a post_action_time")
    }

    pub fn action_duration(&self) -> Duration {
        self.post_action_time()
            .duration_since(self.pre_action_time)
    }

    pub fn into_err_reply(&mut self, errno: Errno) {
        self.reply = Some(Reply::Err(errno));
    }
}
