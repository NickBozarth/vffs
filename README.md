# vffs — Virtual Fault-Injection File System

`vffs` is a user-space passthrough filesystem in Rust built on [`fuser`](https://github.com/cberner/fuser). It proxies an existing source directory onto a mountpoint and evaluates pre-action and post-action rules to inject delays, errors, or byte-level read corruption when testing applications against degraded or failing filesystems along with support for custom modifications.

## How It Works

`vffs` sits between client processes and a backing directory (`source_dir`) mounted at `out_dir`:

1. **Filesystem Proxy (`src/fs/`)**: Mimics an existing host directory. Any action done to either the proxy directory or to the host directory will appear within the other one as well.
2. **Rule Engine (`src/rules/`)**: Evaluates a rule set to find any matching rules. If any rule is found, its modifications act upon the given action to modify a set of context.
3. **CLI & Config (`src/args.rs`)**: The provided CLI takes a `source_dir` and an `out_dir`, resolves a YAML rules file, and mounts the filesystem.

## Supported Conditions & Actions

Built-in callbacks in `src/rules/default_callbacks.rs` currently include:

#### Conditions
  - `ActionType` — match a specific `ActionType`, `All`, or `None`.
  - `Probability` — fire probabilistically using a percentage (`0.0..=100.0`) and `RngSeed`.
  - `MatchPid`, `MatchUid`, `MatchGid` — filter by caller PID/TID, UID, or GID.
  - `MatchProcessName` — match the caller's `/proc/<pid>/comm` name.
  - `IsErr`, `ErrMatches` — inspect post-action replies for any error or a specific errno.
  - `DurationOver` — trigger when an action exceeds a `TimeAmount` duration.
#### Modifications & Logging
  - `DelayBy`, `DelayByPct` — sleep for a fixed duration or a percentage of action duration.
  - `FailWithCode` — return a specified errno.
  - `MapErrno` — rewrite a matching error code to another errno.
  - `RorruptRead` — bit-flip bytes in `Reply::Read` buffers at a given probability.
  - `LogActionCtx`, `LogActionType`, `LogReply`, `LogCtx` — print action, reply, or context debug output.
#### Supported Actions
  - `Lookup` — Resolves a child entry name inside a directory to its `INodeNo` and attributes
  - `Forget` — Decrements the kernel's reference count for a given `INodeNo`
  - `Getattr` — Fetches file or directory metadata and attributes
  - `Setattr` — Updates a file or directory's metadata or attributes
  - `Readdir` — List directory entries along with simple descriptors
  - `Open` — Open a file for access and allocates a `FileHandle`
  - `Read` — Reads bytes from an open file
  - `Write` — Writes bytes to an open file
  - `Release` — Closes and cleans up open file handle
  - `Fsync` — Flushes buffered file data

## Custom Rule Set

All rules come in a defined structure

```yaml
Rule Name:
  rule_type: <RuleType>
  condition_match_type: <ConditionMatchType>
  conditions:
  - <Condition>
  modifications:
  - <Modification>
```

### RuleType
- PreAction — A rule designated to act on information before the action is evaluated. Can be used to do things like inspect and modify incoming arguments.
- PostAction — A rule designated to act on information after the action is evaluated. Can be used to do things like inspect and modify the reply to the user.

### ConditionMatchType
- MatchAll — (Default) Checks that all conditions are true 
- MatchOne — Checks that one or more conditions are true
- DenyMatchAll — Checks that at least one condition is false
- DenyMatchOne — Checks that all conditions are false

### Condition
Conditions contain a named condition and the arguments. Named conditions fall into 4 categories
- A condition from predefined conditions [Jump to Conditions](#conditions)
- A predefined set of logic gates (all 16 binary logic gates and ANY and ALL)
- TRUE or FALSE
- Custom conditions

#### Predefined Conditions
These may be used as shown below
```yaml
conditions:
  # Condition with 0 arguments
- !TRUE
  # Condition with single argument
- !ActionType Read
  # Condition with 2 arguments
- !Probability
  - 50.0
  - !Seeded 1234
```

#### Logic Gates
```yaml
conditions:
- !OR
  - !TRUE
  - !FALSE
- !ANY
  - !FALSE
  - !FALSE
  - !FALSE
  - !TRUE
```


### Modification
Modifications contain a named modification and the arguments. Named modifications fall into 2 categories
- A modification from predefined modifications [Jump to Modifications](#modifications)
- Custom modifications

#### Predefined Modifications
These may be used as shown below
```yaml
modifications:
  # Modification with 0 arguments
- !LogActionType
  # Modification with single argument
- !FailWithCode 12
  # Modification with 2 arguments
- !MapErrno
  - 1
  - 2
```




## Building and Running

Requires Linux with FUSE support (`/dev/fuse` and `fusermount`).

```bash
cargo run -- -s <path-to-host-directory> -o <path-to-proxy-directory> [-r <path-to-rules-file>]
