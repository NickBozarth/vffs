# vffs (Virtual Fault-injection File System)

`vffs` is a user-space filesystem used to test applications. Built on fuser (a rewrite of FUSE in Rust), it proxies an existing directory and captures incoming requests, checks the request against a set of conditions, and applies transformations. This allows for cases such as a slow or failing filesystem to be simulated inside an existing system.
