// This gets generated from async.rs by:
// - removing all async
// - removing all .await
// - replacing embedded-hal-async with embedded-hal
include!(concat!(env!("OUT_DIR"), "/de-asynced.rs"));
